//! The `HandshakePolicy` port: how Transport asks Security whether a handshake
//! may proceed.
//!
//! Transport owns the socket and the TLS machinery, but never the trust
//! decision. During the QUIC handshake the custom certificate verifiers compute
//! the peer's certificate fingerprint and call into this port; the Runtime
//! implements it over Security's trust store (allowlist + TOFU pins).

use omni_protocol::Fingerprint;

/// Why a handshake was refused by policy. The reason is surfaced in the TLS
/// alert/log, so it must never contain key material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyViolation {
    reason: String,
}

impl PolicyViolation {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// A human-readable explanation of the refusal.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "handshake refused: {}", self.reason)
    }
}

impl std::error::Error for PolicyViolation {}

/// Decides, during the TLS handshake, whether a peer's certificate is
/// acceptable. Implemented by the Runtime over Security's trust policy;
/// enforced by Transport inside its rustls certificate verifiers.
pub trait HandshakePolicy: Send + Sync {
    /// Outbound: this machine dialed `host` and the server presented a
    /// certificate with `fingerprint`. TOFU lives here: a pinned fingerprint
    /// for `host` that differs from this one must be refused.
    fn authorize_server(&self, host: &str, fingerprint: Fingerprint)
    -> Result<(), PolicyViolation>;

    /// Inbound: a client presented a certificate with `fingerprint`. Unknown
    /// peers may be admitted here so the user can be asked to accept or reject
    /// them; peers the user has blocked must be refused outright.
    fn authorize_client(&self, fingerprint: Fingerprint) -> Result<(), PolicyViolation>;
}
