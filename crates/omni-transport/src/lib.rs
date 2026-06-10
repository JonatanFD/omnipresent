//! Transport: moves encoded messages between machines — the pipe.
//!
//! Carries Protocol messages over a **QUIC** connection (TLS 1.3 over UDP). QUIC
//! provides the confidentiality, integrity, mutual authentication, and replay
//! protection; input events travel as unreliable QUIC datagrams (RFC 9221) so a
//! lost packet is dropped rather than retransmitted, keeping latency low. UDP
//! only — never a TCP fallback.
//!
//! The crate exposes the [`SecureChannel`] port (an established per-peer
//! connection), the [`Transport`] glue that frames messages over it, and the
//! production QUIC adapter over [`quinn`](https://docs.rs/quinn): a
//! [`QuicEndpoint`] that dials and listens on one UDP socket, with the
//! [`HandshakePolicy`] port enforced inside custom rustls certificate
//! verifiers. A loopback channel stands in for QUIC in unit tests.

pub mod channel;
pub mod endpoint;
pub mod policy;
pub mod quic;
pub mod transport;

mod tls;

pub use channel::{LoopbackChannel, SecureChannel};
pub use endpoint::Endpoint;
pub use policy::{HandshakePolicy, PolicyViolation};
pub use quic::{
    ControlReceiver, ControlSender, ControlStream, QuicConnection, QuicEndpoint, QuicError,
};
pub use tls::ALPN;
pub use transport::{Transport, TransportError};
