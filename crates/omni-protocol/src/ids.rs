//! Shared identifiers exchanged between machines.
//!
//! These are thin value objects: they give meaning to otherwise interchangeable
//! integers and byte arrays so the type system keeps, say, a [`SessionId`] from
//! being passed where a [`MachineId`] is expected. How they are generated is the
//! concern of other modules (Session mints session ids, Security derives
//! fingerprints); Protocol only defines what they are on the wire.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Identifies a physical machine taking part in the topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MachineId(u64);

impl MachineId {
    /// Wraps a raw machine identifier.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The underlying value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Identifies a known peer. Derived by Security from the peer's certificate, so
/// the same peer keeps the same id across reconnects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(u64);

impl PeerId {
    /// Wraps a raw peer identifier.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The underlying value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Identifies a single active session between two machines. Minted by Session
/// when a connection is accepted; unique for the lifetime of that session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(u128);

impl SessionId {
    /// Wraps a raw session identifier.
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// The underlying value.
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// A peer's certificate fingerprint: the SHA-256 of its certificate, the value
/// pinned on first use (TOFU) and compared on every later connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Wraps the 32 raw bytes of a SHA-256 digest.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Renders the fingerprint as lowercase hex, the form shown to users and pinned
/// in the trust store.
impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_their_raw_value() {
        assert_eq!(MachineId::new(7).value(), 7);
        assert_eq!(PeerId::new(42).value(), 42);
        assert_eq!(SessionId::new(u128::MAX).value(), u128::MAX);
    }

    #[test]
    fn fingerprint_exposes_its_bytes() {
        let bytes = [0xABu8; 32];
        let fp = Fingerprint::from_bytes(bytes);
        assert_eq!(fp.as_bytes(), &bytes);
    }

    #[test]
    fn fingerprint_displays_as_lowercase_hex() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0x0a;
        bytes[31] = 0xff;
        let shown = Fingerprint::from_bytes(bytes).to_string();

        assert_eq!(shown.len(), 64);
        assert!(shown.starts_with("0a"));
        assert!(shown.ends_with("ff"));
    }

    #[test]
    fn equal_fingerprints_compare_equal() {
        // TOFU depends on this: the same certificate must pin the same value.
        assert_eq!(
            Fingerprint::from_bytes([1; 32]),
            Fingerprint::from_bytes([1; 32]),
        );
        assert_ne!(
            Fingerprint::from_bytes([1; 32]),
            Fingerprint::from_bytes([2; 32]),
        );
    }
}
