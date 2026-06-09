//! Transport: moves encoded messages between machines — the pipe.
//!
//! Carries Protocol messages over a **QUIC** connection (TLS 1.3 over UDP). QUIC
//! provides the confidentiality, integrity, mutual authentication, and replay
//! protection; input events travel as unreliable QUIC datagrams (RFC 9221) so a
//! lost packet is dropped rather than retransmitted, keeping latency low. UDP
//! only — never a TCP fallback.
//!
//! The crate exposes the [`SecureChannel`] port (an established per-peer
//! connection) and the [`Transport`] glue that frames messages over it. The real
//! adapter wraps [`quinn`](https://docs.rs/quinn) and is a planned follow-up; for
//! now a loopback channel exercises the framing and pipelines in tests.

pub mod channel;
pub mod endpoint;
pub mod transport;

pub use channel::{LoopbackChannel, SecureChannel};
pub use endpoint::Endpoint;
pub use transport::{Transport, TransportError};
