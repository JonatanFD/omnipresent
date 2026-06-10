//! Live two-endpoint tests of the QUIC adapter over localhost: mutual TLS with
//! self-signed identities, policy enforcement during the handshake, datagrams,
//! and the control stream.

use omni_protocol::input::{Action, KeyCode, Modifiers};
use omni_protocol::{ControlMessage, Fingerprint, InputEvent, Message, ScreenSize, SessionId};
use omni_security::{LocalIdentity, generate_identity};
use omni_transport::{
    HandshakePolicy, PolicyViolation, QuicEndpoint, SecureChannel, Transport, TransportError,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// A policy that admits every peer — the "both sides already trust each other"
/// case, letting the tests focus on the channel itself.
struct AllowAll;

impl HandshakePolicy for AllowAll {
    fn authorize_server(&self, _host: &str, _fp: Fingerprint) -> Result<(), PolicyViolation> {
        Ok(())
    }

    fn authorize_client(&self, _fp: Fingerprint) -> Result<(), PolicyViolation> {
        Ok(())
    }
}

/// A policy that pins exactly one acceptable fingerprint in each direction.
struct PinnedOnly {
    server: Fingerprint,
    client: Fingerprint,
}

impl HandshakePolicy for PinnedOnly {
    fn authorize_server(&self, _host: &str, fp: Fingerprint) -> Result<(), PolicyViolation> {
        if fp == self.server {
            Ok(())
        } else {
            Err(PolicyViolation::new("fingerprint mismatch"))
        }
    }

    fn authorize_client(&self, fp: Fingerprint) -> Result<(), PolicyViolation> {
        if fp == self.client {
            Ok(())
        } else {
            Err(PolicyViolation::new("not allowed"))
        }
    }
}

fn localhost() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn endpoint(identity: &LocalIdentity, policy: impl HandshakePolicy + 'static) -> QuicEndpoint {
    QuicEndpoint::bind(localhost(), identity, Arc::new(policy)).expect("bind endpoint")
}

fn key_press(session: u128) -> Message {
    Message::Input {
        session: SessionId::new(session),
        event: InputEvent::Key {
            code: KeyCode::new(0x04),
            action: Action::Press,
            modifiers: Modifiers::SHIFT,
        },
    }
}

#[tokio::test]
async fn two_endpoints_exchange_datagrams_both_ways() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let alpha = endpoint(&alpha_id, AllowAll);
    let beta = endpoint(&beta_id, AllowAll);
    let beta_addr = beta.local_addr().unwrap();

    let (dialed, accepted) = tokio::join!(alpha.connect(beta_addr, "localhost"), async {
        beta.accept().await.expect("incoming connection")
    },);
    let dialed = dialed.expect("connect");
    let accepted = accepted.expect("accept");

    // Each side sees the other's certificate fingerprint.
    assert_eq!(dialed.peer_fingerprint(), beta_id.fingerprint());
    assert_eq!(accepted.peer_fingerprint(), alpha_id.fingerprint());

    // Frame Protocol messages over the connection in both directions.
    let mut controller = Transport::new(dialed);
    let mut target = Transport::new(accepted);

    controller.send(&key_press(1)).expect("send datagram");
    let received = recv_message(&mut target).await;
    assert_eq!(received, key_press(1));

    target.send(&key_press(2)).expect("send reply");
    let received = recv_message(&mut controller).await;
    assert_eq!(received, key_press(2));
}

/// Polls a transport until its datagram arrives (datagrams are unreliable but
/// loopback delivery is just asynchronous, not lossy).
async fn recv_message<C>(transport: &mut Transport<C>) -> Message
where
    C: SecureChannel,
    C::Error: std::fmt::Debug,
{
    for _ in 0..100 {
        match transport.recv() {
            Ok(Some(message)) => return message,
            Ok(None) => tokio::time::sleep(Duration::from_millis(10)).await,
            Err(TransportError::Channel(e)) => panic!("channel failed: {e:?}"),
            Err(TransportError::Codec(e)) => panic!("codec failed: {e}"),
        }
    }
    panic!("no datagram arrived within a second");
}

#[tokio::test]
async fn control_messages_ride_the_reliable_stream() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let alpha = endpoint(&alpha_id, AllowAll);
    let beta = endpoint(&beta_id, AllowAll);
    let beta_addr = beta.local_addr().unwrap();

    let (dialed, accepted) = tokio::join!(alpha.connect(beta_addr, "localhost"), async {
        beta.accept().await.expect("incoming connection")
    },);
    let dialed = dialed.unwrap();
    let accepted = accepted.unwrap();

    let request = Message::Control(ControlMessage::ConnectRequest {
        machine: omni_protocol::MachineId::new(7),
        fingerprint: alpha_id.fingerprint(),
        screen: ScreenSize::new(1920, 1080),
    });
    let accept = Message::Control(ControlMessage::Accept {
        session: SessionId::new(42),
        machine: omni_protocol::MachineId::new(9),
        screen: ScreenSize::new(2560, 1440),
    });

    let mut initiator_stream = dialed.open_control().await.expect("open control");
    initiator_stream.send(&request).await.expect("send request");

    let mut target_stream = accepted.accept_control().await.expect("accept control");
    assert_eq!(target_stream.recv().await.unwrap(), Some(request));

    target_stream.send(&accept).await.expect("send accept");
    assert_eq!(initiator_stream.recv().await.unwrap(), Some(accept));

    // Finishing the stream surfaces as a clean end on the other side.
    initiator_stream.finish();
    assert_eq!(target_stream.recv().await.unwrap(), None);
}

#[tokio::test]
async fn a_client_the_server_policy_refuses_cannot_connect() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let intruder_id = generate_identity("intruder").unwrap();

    // Beta only admits alpha's fingerprint, but the intruder dials.
    let beta = endpoint(
        &beta_id,
        PinnedOnly {
            server: alpha_id.fingerprint(),
            client: alpha_id.fingerprint(),
        },
    );
    let beta_addr = beta.local_addr().unwrap();
    let intruder = endpoint(&intruder_id, AllowAll);

    let (dialed, accepted) = tokio::join!(intruder.connect(beta_addr, "localhost"), async {
        // The refused handshake must never yield a connection on beta's side.
        tokio::time::timeout(Duration::from_millis(500), beta.accept())
            .await
            .ok()
    });

    if let Some(Some(result)) = accepted {
        assert!(result.is_err(), "refused client was accepted by the server");
    }

    // In TLS 1.3 the dialer can finish its side of the handshake before the
    // server has validated the client certificate, so the refusal may surface
    // either as a failed connect or as an immediate close.
    if let Ok(connection) = dialed {
        tokio::time::timeout(Duration::from_secs(5), connection.closed())
            .await
            .expect("refused client stayed connected");
    }
}

#[tokio::test]
async fn a_server_with_a_changed_fingerprint_is_refused_by_the_client() {
    let alpha_id = generate_identity("alpha").unwrap();
    let beta_id = generate_identity("beta").unwrap();
    let expected = generate_identity("the-beta-alpha-pinned-before").unwrap();

    // Alpha pinned a different certificate for this host: TOFU must refuse.
    let alpha = endpoint(
        &alpha_id,
        PinnedOnly {
            server: expected.fingerprint(),
            client: beta_id.fingerprint(),
        },
    );
    let beta = endpoint(&beta_id, AllowAll);
    let beta_addr = beta.local_addr().unwrap();

    let dialed = alpha.connect(beta_addr, "localhost").await;

    assert!(
        dialed.is_err(),
        "mismatched server fingerprint was accepted"
    );
}
