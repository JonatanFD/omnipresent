//! The `TrustStore` port: persisting the allowlist and pinned fingerprints.
//!
//! Real adapters (the on-disk trust store under the config dir) live in the
//! Runtime; this module ships an in-memory adapter for tests and for running
//! without persistence.

use crate::trust::AllowList;
use omni_protocol::{Fingerprint, PeerId};
use std::collections::HashMap;
use std::convert::Infallible;

/// Persists the data the trust policy needs: who is allowed, and which
/// fingerprint is pinned for each peer.
pub trait TrustStore {
    /// What can go wrong reading or writing for this particular backend.
    type Error;

    /// The current allowlist.
    fn allowlist(&self) -> Result<AllowList, Self::Error>;

    /// The fingerprint pinned for a peer, or `None` if it has never been pinned.
    fn pinned(&self, peer: PeerId) -> Result<Option<Fingerprint>, Self::Error>;

    /// Adds a peer to the allowlist.
    fn allow(&mut self, peer: PeerId) -> Result<(), Self::Error>;

    /// Removes a peer from the allowlist.
    fn remove_allowed(&mut self, peer: PeerId) -> Result<(), Self::Error>;

    /// Pins a peer's fingerprint, replacing any previous one.
    fn pin(&mut self, peer: PeerId, fingerprint: Fingerprint) -> Result<(), Self::Error>;

    /// Drops a peer's pinned fingerprint, if any.
    fn unpin(&mut self, peer: PeerId) -> Result<(), Self::Error>;
}

/// A `TrustStore` kept entirely in memory. Its operations cannot fail, so its
/// error type is [`Infallible`].
#[derive(Debug, Default)]
pub struct InMemoryTrustStore {
    allowlist: AllowList,
    pins: HashMap<PeerId, Fingerprint>,
}

impl TrustStore for InMemoryTrustStore {
    type Error = Infallible;

    fn allowlist(&self) -> Result<AllowList, Self::Error> {
        Ok(self.allowlist.clone())
    }

    fn pinned(&self, peer: PeerId) -> Result<Option<Fingerprint>, Self::Error> {
        Ok(self.pins.get(&peer).copied())
    }

    fn allow(&mut self, peer: PeerId) -> Result<(), Self::Error> {
        self.allowlist.allow(peer);
        Ok(())
    }

    fn remove_allowed(&mut self, peer: PeerId) -> Result<(), Self::Error> {
        self.allowlist.remove(peer);
        Ok(())
    }

    fn pin(&mut self, peer: PeerId, fingerprint: Fingerprint) -> Result<(), Self::Error> {
        self.pins.insert(peer, fingerprint);
        Ok(())
    }

    fn unpin(&mut self, peer: PeerId) -> Result<(), Self::Error> {
        self.pins.remove(&peer);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEER: PeerId = PeerId::new(42);

    #[test]
    fn pins_round_trip() {
        let mut store = InMemoryTrustStore::default();
        assert_eq!(store.pinned(PEER).unwrap(), None);

        let fingerprint = Fingerprint::from_bytes([5; 32]);
        store.pin(PEER, fingerprint).unwrap();
        assert_eq!(store.pinned(PEER).unwrap(), Some(fingerprint));

        store.unpin(PEER).unwrap();
        assert_eq!(store.pinned(PEER).unwrap(), None);
    }

    #[test]
    fn allowlist_changes_are_visible() {
        let mut store = InMemoryTrustStore::default();
        store.allow(PEER).unwrap();
        assert!(store.allowlist().unwrap().contains(PEER));

        store.remove_allowed(PEER).unwrap();
        assert!(!store.allowlist().unwrap().contains(PEER));
    }
}
