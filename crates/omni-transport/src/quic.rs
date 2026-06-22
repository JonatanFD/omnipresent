//! The production [`SecureChannel`] adapter: QUIC over [`quinn`].
//!
//! One [`QuicEndpoint`] owns one UDP socket and acts as both dialer and
//! listener — any machine can initiate or receive connections (the roles are
//! dynamic). Every connection is mutually authenticated TLS 1.3; the
//! [`HandshakePolicy`] port is enforced inside the certificate verifiers, so an
//! unauthorized peer never gets past the handshake. Input events ride
//! unreliable QUIC datagrams (RFC 9221); session signalling rides one reliable
//! bidirectional stream framed by [`ControlStream`].

use crate::channel::SecureChannel;
use crate::endpoint::Endpoint;
use crate::policy::HandshakePolicy;
use crate::tls;
use bytes::Bytes;
use omni_protocol::{CodecError, Fingerprint, MAX_CLIPBOARD_BYTES, Message, decode, encode};
use omni_security::{LocalIdentity, fingerprint_of};
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::pki_types::CertificateDer;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// How long a silent connection lives. QUIC keep-alives ping every
/// [`KEEP_ALIVE`], so only a genuinely dead peer reaches the timeout.
const IDLE_TIMEOUT: Duration = Duration::from_secs(15);
const KEEP_ALIVE: Duration = Duration::from_secs(5);

/// How many bytes of unsent input datagrams quinn may buffer. Input is
/// real-time: when the link congests, a fresh cursor position makes every
/// older one stale, so a deep queue only adds lag. quinn's `send_datagram`
/// drops the oldest buffered datagrams to admit a new one, so a shallow buffer
/// bounds the worst-case cursor lag to a few packets instead of the 1 MiB
/// default (tens of thousands of stale positions). A Protocol input message is
/// only tens of bytes, so this still holds a healthy burst.
const DATAGRAM_SEND_BUFFER: usize = 16 * 1024;

/// The largest control frame we will read. Session signalling is tiny, but the
/// control stream also carries clipboard payloads — including images, which are
/// far bigger than a control message — so the limit must admit a full clipboard
/// payload (capped at [`MAX_CLIPBOARD_BYTES`]) plus the small postcard framing
/// overhead. Anything beyond that is a protocol violation, not a message.
const MAX_CONTROL_FRAME: usize = MAX_CLIPBOARD_BYTES + 1024;

/// Why a QUIC operation failed.
#[derive(Debug)]
pub enum QuicError {
    /// Binding the UDP socket failed.
    Bind(std::io::Error),
    /// Building the TLS or QUIC configuration failed.
    Config(String),
    /// Starting an outbound connection failed (bad address, closed endpoint...).
    Connect(quinn::ConnectError),
    /// An established connection failed or was closed.
    Connection(quinn::ConnectionError),
    /// Sending a datagram failed.
    Datagram(quinn::SendDatagramError),
    /// Writing to the control stream failed.
    Write(quinn::WriteError),
    /// Reading from the control stream failed.
    Read(quinn::ReadExactError),
    /// A message could not be encoded or decoded.
    Codec(CodecError),
    /// The peer sent a control frame larger than [`MAX_CONTROL_FRAME`].
    ControlFrameTooLarge(usize),
    /// The handshake completed without a peer certificate — should be
    /// impossible with mandatory mTLS, and is treated as fatal.
    MissingPeerCertificate,
}

impl std::fmt::Display for QuicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuicError::Bind(e) => write!(f, "could not bind UDP socket: {e}"),
            QuicError::Config(e) => write!(f, "invalid QUIC/TLS configuration: {e}"),
            QuicError::Connect(e) => write!(f, "could not start connection: {e}"),
            QuicError::Connection(e) => write!(f, "connection failed: {e}"),
            QuicError::Datagram(e) => write!(f, "could not send datagram: {e}"),
            QuicError::Write(e) => write!(f, "control stream write failed: {e}"),
            QuicError::Read(e) => write!(f, "control stream read failed: {e}"),
            QuicError::Codec(e) => write!(f, "codec error: {e}"),
            QuicError::ControlFrameTooLarge(len) => {
                write!(f, "control frame of {len} bytes exceeds the limit")
            }
            QuicError::MissingPeerCertificate => {
                write!(f, "peer presented no certificate")
            }
        }
    }
}

impl std::error::Error for QuicError {}

/// Shared transport tuning for both directions of every connection.
fn transport_config() -> Arc<quinn::TransportConfig> {
    let mut config = quinn::TransportConfig::default();
    config.keep_alive_interval(Some(KEEP_ALIVE));
    config.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(IDLE_TIMEOUT).expect("idle timeout fits a varint"),
    ));
    // Keep the input (datagram) path tuned for latency, not throughput.
    config.datagram_send_buffer_size(DATAGRAM_SEND_BUFFER);
    // BBR keeps the bottleneck queue (and thus added latency) small under
    // competing traffic, where the default loss-based controller lets a queue
    // build up first — exactly the high-traffic case that lagged the cursor.
    config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
    Arc::new(config)
}

/// One UDP socket speaking QUIC in both directions: it listens for inbound
/// connections and dials outbound ones, presenting the same identity either
/// way. Must be created and used inside a tokio runtime.
#[derive(Debug)]
pub struct QuicEndpoint {
    endpoint: quinn::Endpoint,
}

impl QuicEndpoint {
    /// Binds `addr` and prepares both server and client configurations from
    /// this machine's identity and the trust policy.
    pub fn bind(
        addr: SocketAddr,
        identity: &LocalIdentity,
        policy: Arc<dyn HandshakePolicy>,
    ) -> Result<Self, QuicError> {
        let server_tls = tls::server_config(identity, policy.clone())
            .map_err(|e| QuicError::Config(e.to_string()))?;
        let server_crypto =
            QuicServerConfig::try_from(server_tls).map_err(|e| QuicError::Config(e.to_string()))?;
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server_crypto));
        server_config.transport_config(transport_config());

        let client_tls =
            tls::client_config(identity, policy).map_err(|e| QuicError::Config(e.to_string()))?;
        let client_crypto =
            QuicClientConfig::try_from(client_tls).map_err(|e| QuicError::Config(e.to_string()))?;
        let mut client_config = quinn::ClientConfig::new(Arc::new(client_crypto));
        client_config.transport_config(transport_config());

        let mut endpoint = quinn::Endpoint::server(server_config, addr).map_err(QuicError::Bind)?;
        endpoint.set_default_client_config(client_config);
        Ok(Self { endpoint })
    }

    /// The address the UDP socket actually bound (resolves port 0).
    pub fn local_addr(&self) -> Result<SocketAddr, QuicError> {
        self.endpoint.local_addr().map_err(QuicError::Bind)
    }

    /// Dials a peer. `host` is what the user asked to connect to; it is the
    /// TOFU key the policy uses to detect a changed server certificate.
    pub async fn connect(&self, addr: SocketAddr, host: &str) -> Result<QuicConnection, QuicError> {
        let connecting = self
            .endpoint
            .connect(addr, host)
            .map_err(QuicError::Connect)?;
        let connection = connecting.await.map_err(QuicError::Connection)?;
        QuicConnection::wrap(connection)
    }

    /// Waits for the next inbound connection. Returns `None` once the endpoint
    /// is closed. Peers that fail the policy check never appear here — their
    /// handshake is refused.
    pub async fn accept(&self) -> Option<Result<QuicConnection, QuicError>> {
        let incoming = self.endpoint.accept().await?;
        Some(
            async {
                let connection = incoming.await.map_err(QuicError::Connection)?;
                QuicConnection::wrap(connection)
            }
            .await,
        )
    }

    /// Closes the endpoint and every connection on it.
    pub fn close(&self) {
        self.endpoint.close(quinn::VarInt::from_u32(0), b"shutdown");
    }
}

/// An established, mutually authenticated QUIC connection to one peer. This is
/// the production [`SecureChannel`].
#[derive(Debug)]
pub struct QuicConnection {
    connection: quinn::Connection,
    peer_fingerprint: Fingerprint,
    /// Datagrams received by the background pump, waiting to be polled.
    datagrams: mpsc::UnboundedReceiver<Bytes>,
}

impl QuicConnection {
    /// Wraps an established quinn connection: extracts the peer's certificate
    /// fingerprint and starts the datagram pump.
    fn wrap(connection: quinn::Connection) -> Result<Self, QuicError> {
        let peer_fingerprint = peer_fingerprint(&connection)?;

        let (tx, datagrams) = mpsc::unbounded_channel();
        let pump = connection.clone();
        tokio::spawn(async move {
            // Ends when the connection closes or the receiver is dropped.
            while let Ok(payload) = pump.read_datagram().await {
                if tx.send(payload).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            connection,
            peer_fingerprint,
            datagrams,
        })
    }

    /// The fingerprint of the certificate the peer presented — the identity
    /// everything above this layer keys trust on.
    pub fn peer_fingerprint(&self) -> Fingerprint {
        self.peer_fingerprint
    }

    /// The peer's network address.
    pub fn remote_address(&self) -> SocketAddr {
        self.connection.remote_address()
    }

    /// Opens the reliable control stream (initiator side).
    pub async fn open_control(&self) -> Result<ControlStream, QuicError> {
        let (send, recv) = self
            .connection
            .open_bi()
            .await
            .map_err(QuicError::Connection)?;
        Ok(ControlStream {
            sender: ControlSender { send },
            receiver: ControlReceiver { recv },
        })
    }

    /// Accepts the reliable control stream the initiator opened (target side).
    pub async fn accept_control(&self) -> Result<ControlStream, QuicError> {
        let (send, recv) = self
            .connection
            .accept_bi()
            .await
            .map_err(QuicError::Connection)?;
        Ok(ControlStream {
            sender: ControlSender { send },
            receiver: ControlReceiver { recv },
        })
    }

    /// Waits for the next datagram. Returns `None` once the connection closes.
    pub async fn recv_datagram_async(&mut self) -> Option<Vec<u8>> {
        self.datagrams.recv().await.map(|b| b.to_vec())
    }

    /// Resolves when the connection closes, with the reason.
    pub async fn closed(&self) -> quinn::ConnectionError {
        self.connection.closed().await
    }

    /// Closes the connection immediately.
    pub fn close(&self) {
        self.connection.close(quinn::VarInt::from_u32(0), b"done");
    }
}

/// The peer's certificate fingerprint, from the TLS session.
fn peer_fingerprint(connection: &quinn::Connection) -> Result<Fingerprint, QuicError> {
    let identity = connection
        .peer_identity()
        .ok_or(QuicError::MissingPeerCertificate)?;
    let certs = identity
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| QuicError::MissingPeerCertificate)?;
    let end_entity = certs.first().ok_or(QuicError::MissingPeerCertificate)?;
    Ok(fingerprint_of(end_entity.as_ref()))
}

impl SecureChannel for QuicConnection {
    type Error = QuicError;

    fn peer(&self) -> Endpoint {
        Endpoint::new(self.connection.remote_address())
    }

    fn send_datagram(&mut self, payload: &[u8]) -> Result<(), Self::Error> {
        self.connection
            .send_datagram(Bytes::copy_from_slice(payload))
            .map_err(QuicError::Datagram)
    }

    fn recv_datagram(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        match self.datagrams.try_recv() {
            Ok(payload) => Ok(Some(payload.to_vec())),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(QuicError::Connection(quinn::ConnectionError::LocallyClosed))
            }
        }
    }
}

impl crate::transport::Transport<QuicConnection> {
    /// Waits for the next datagram message. Returns `None` once the connection
    /// closes — the async sibling of [`Transport::recv`](crate::Transport::recv).
    pub async fn recv_async(
        &mut self,
    ) -> Result<Option<Message>, crate::TransportError<QuicError>> {
        match self.channel_mut().recv_datagram_async().await {
            Some(payload) => decode(&payload)
                .map(Some)
                .map_err(crate::TransportError::Codec),
            None => Ok(None),
        }
    }
}

/// The reliable signalling stream: length-prefixed Protocol [`Message`]s over
/// one QUIC bidirectional stream. Loss here is retransmitted by QUIC — exactly
/// what connect/accept/disconnect need (and what input events must avoid).
#[derive(Debug)]
pub struct ControlStream {
    sender: ControlSender,
    receiver: ControlReceiver,
}

impl ControlStream {
    /// Sends one message, framed with a 4-byte big-endian length.
    pub async fn send(&mut self, message: &Message) -> Result<(), QuicError> {
        self.sender.send(message).await
    }

    /// Receives the next message. Returns `None` when the peer finished the
    /// stream cleanly between frames.
    pub async fn recv(&mut self) -> Result<Option<Message>, QuicError> {
        self.receiver.recv().await
    }

    /// Finishes the send side, telling the peer no more control messages come.
    pub fn finish(&mut self) {
        self.sender.finish();
    }

    /// Splits the stream so sending and receiving can live in different tasks.
    pub fn split(self) -> (ControlSender, ControlReceiver) {
        (self.sender, self.receiver)
    }
}

/// The sending half of a [`ControlStream`].
#[derive(Debug)]
pub struct ControlSender {
    send: quinn::SendStream,
}

impl ControlSender {
    /// Sends one message, framed with a 4-byte big-endian length.
    pub async fn send(&mut self, message: &Message) -> Result<(), QuicError> {
        let payload = encode(message).map_err(QuicError::Codec)?;
        let len = u32::try_from(payload.len())
            .map_err(|_| QuicError::ControlFrameTooLarge(payload.len()))?;
        self.send
            .write_all(&len.to_be_bytes())
            .await
            .map_err(QuicError::Write)?;
        self.send
            .write_all(&payload)
            .await
            .map_err(QuicError::Write)?;
        Ok(())
    }

    /// Finishes the send side, telling the peer no more control messages come.
    pub fn finish(&mut self) {
        // Finishing an already-closed stream is harmless.
        let _ = self.send.finish();
    }
}

/// The receiving half of a [`ControlStream`].
#[derive(Debug)]
pub struct ControlReceiver {
    recv: quinn::RecvStream,
}

impl ControlReceiver {
    /// Receives the next message. Returns `None` when the peer finished the
    /// stream cleanly between frames.
    pub async fn recv(&mut self) -> Result<Option<Message>, QuicError> {
        let mut len_buf = [0u8; 4];
        match self.recv.read_exact(&mut len_buf).await {
            Ok(()) => {}
            // A clean end of stream before a frame starts is a normal close.
            Err(quinn::ReadExactError::FinishedEarly(0)) => return Ok(None),
            Err(e) => return Err(QuicError::Read(e)),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_CONTROL_FRAME {
            return Err(QuicError::ControlFrameTooLarge(len));
        }
        let mut payload = vec![0u8; len];
        self.recv
            .read_exact(&mut payload)
            .await
            .map_err(QuicError::Read)?;
        let message = decode(&payload).map_err(QuicError::Codec)?;
        Ok(Some(message))
    }
}
