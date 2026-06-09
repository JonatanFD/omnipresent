//! The `CertProvider` port: supplying this machine's own certificate and key.
//!
//! The key material is held opaquely. [`LocalIdentity`]'s `Debug` redacts the
//! private key (and certificate bytes) so it can never leak into logs — only the
//! fingerprint is ever shown, honouring the "key material never logged" rule.

use omni_protocol::Fingerprint;
use std::fmt;

/// This machine's TLS identity: its certificate, its private key, and the
/// fingerprint of the certificate. The byte buffers are DER-encoded.
#[derive(Clone)]
pub struct LocalIdentity {
    certificate_der: Vec<u8>,
    private_key_der: Vec<u8>,
    fingerprint: Fingerprint,
}

impl LocalIdentity {
    /// Builds an identity from DER-encoded certificate and key bytes and the
    /// certificate's fingerprint.
    pub fn new(
        certificate_der: Vec<u8>,
        private_key_der: Vec<u8>,
        fingerprint: Fingerprint,
    ) -> Self {
        Self {
            certificate_der,
            private_key_der,
            fingerprint,
        }
    }

    /// The certificate's fingerprint — the value a peer pins via TOFU.
    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// The DER-encoded certificate, to present during the handshake.
    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }

    /// The DER-encoded private key. Handed only to the TLS stack; never logged.
    pub fn private_key_der(&self) -> &[u8] {
        &self.private_key_der
    }
}

/// Redacts both key material and certificate bytes; shows only the fingerprint.
impl fmt::Debug for LocalIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalIdentity")
            .field("fingerprint", &self.fingerprint)
            .field("certificate_der", &"<redacted>")
            .field("private_key_der", &"<redacted>")
            .finish()
    }
}

/// Supplies this machine's identity to whoever needs it for a handshake.
pub trait CertProvider {
    /// What can go wrong loading or generating the identity.
    type Error;

    /// This machine's certificate and key material.
    fn local_identity(&self) -> Result<LocalIdentity, Self::Error>;
}

/// A `CertProvider` holding a fixed identity in memory, for tests and for
/// running with a preloaded certificate.
#[derive(Debug, Clone)]
pub struct InMemoryCertProvider {
    identity: LocalIdentity,
}

impl InMemoryCertProvider {
    pub fn new(identity: LocalIdentity) -> Self {
        Self { identity }
    }
}

impl CertProvider for InMemoryCertProvider {
    type Error = std::convert::Infallible;

    fn local_identity(&self) -> Result<LocalIdentity, Self::Error> {
        Ok(self.identity.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LocalIdentity {
        LocalIdentity::new(
            vec![0xC0, 0xDE],
            vec![0x53, 0x3C, 0x12],
            Fingerprint::from_bytes([1; 32]),
        )
    }

    #[test]
    fn exposes_fingerprint_and_certificate() {
        let id = sample();
        assert_eq!(id.fingerprint(), Fingerprint::from_bytes([1; 32]));
        assert_eq!(id.certificate_der(), &[0xC0, 0xDE]);
    }

    #[test]
    fn debug_never_reveals_key_material() {
        // The private key bytes 0x53, 0x3C, 0x12 must not appear in any form.
        let shown = format!("{:?}", sample());

        assert!(shown.contains("<redacted>"));
        assert!(shown.contains("fingerprint"));
        assert!(!shown.contains("53"));
        assert!(!shown.contains("60")); // 0x3C and 0x12 as decimal, just in case
        assert!(!shown.contains("18"));
    }

    #[test]
    fn provider_returns_the_identity() {
        let provider = InMemoryCertProvider::new(sample());
        let id = provider.local_identity().unwrap();
        assert_eq!(id.fingerprint(), Fingerprint::from_bytes([1; 32]));
    }
}
