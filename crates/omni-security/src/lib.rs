//! Security: the trust authority. Owns policy, not the socket.
//!
//! Decides which peers may establish a channel by combining an explicit
//! allowlist with Trust-On-First-Use fingerprint pinning, and supplies this
//! machine's own certificate identity. It never opens a socket or speaks TLS —
//! it tells Transport who and what is acceptable, and Transport calls in to
//! authorize a handshake.
//!
//! The real cryptography (certificate generation, the DTLS handshake) lives in
//! Transport's adapter once the DTLS stack is chosen; everything here is pure
//! policy and is fully unit-tested with in-memory adapters.

pub mod identity;
pub mod store;
pub mod trust;

pub use identity::{CertProvider, InMemoryCertProvider, LocalIdentity};
pub use store::{InMemoryTrustStore, TrustStore};
pub use trust::{AllowList, PeerIdentity, TrustAuthority, TrustDecision, evaluate};
