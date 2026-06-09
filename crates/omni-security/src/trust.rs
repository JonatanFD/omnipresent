//! The trust policy: deciding whether a peer may establish a channel.
//!
//! Two rules, applied in order:
//!
//! 1. **Allowlist** — only explicitly allowed peers are considered at all.
//! 2. **TOFU** — the first time an allowed peer is seen, its certificate
//!    fingerprint is pinned; every later connection must present the same one.
//!
//! [`evaluate`] is a pure function over this state, so the decision is trivial to
//! test. [`TrustAuthority`] wraps a [`TrustStore`](crate::store::TrustStore) to
//! apply it against persisted state and to record approvals.

use crate::store::TrustStore;
use omni_protocol::{Fingerprint, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The set of peers explicitly permitted to connect. Anything not in here is
/// rejected at the boundary before any input is processed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowList {
    peers: HashSet<PeerId>,
}

impl AllowList {
    /// An empty allowlist that permits no one.
    pub fn new() -> Self {
        Self::default()
    }

    /// Permits a peer. Returns whether it was newly added.
    pub fn allow(&mut self, peer: PeerId) -> bool {
        self.peers.insert(peer)
    }

    /// Revokes a peer. Returns whether it had been present.
    pub fn remove(&mut self, peer: PeerId) -> bool {
        self.peers.remove(&peer)
    }

    /// Whether a peer is permitted.
    pub fn contains(&self, peer: PeerId) -> bool {
        self.peers.contains(&peer)
    }

    /// Whether no peer is permitted.
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// The permitted peers, in no particular order.
    pub fn peers(&self) -> impl Iterator<Item = PeerId> + '_ {
        self.peers.iter().copied()
    }
}

/// How a peer is identified for a trust decision: a stable handle plus the
/// certificate fingerprint it presented on this connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerIdentity {
    pub peer: PeerId,
    pub fingerprint: Fingerprint,
}

impl PeerIdentity {
    pub const fn new(peer: PeerId, fingerprint: Fingerprint) -> Self {
        Self { peer, fingerprint }
    }
}

/// The outcome of evaluating a peer against the allowlist and pinned
/// fingerprints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustDecision {
    /// Allowed and the fingerprint matches the pinned one. Proceed.
    Trusted,
    /// Allowed but never seen before. The caller may proceed only after a person
    /// approves, then pin this fingerprint (see [`TrustAuthority::accept`]).
    TrustOnFirstUse,
    /// Not on the allowlist. Reject before any processing.
    NotAllowed,
    /// Allowed, but the fingerprint differs from the pinned one — a possible
    /// machine-in-the-middle. Reject and alert.
    FingerprintMismatch,
}

impl TrustDecision {
    /// Whether this decision lets a channel proceed without any human action.
    pub const fn is_trusted(self) -> bool {
        matches!(self, TrustDecision::Trusted)
    }
}

/// Applies the trust rules to a peer. Pure: the allowlist and the peer's pinned
/// fingerprint (if any) are passed in, and a decision comes out.
pub fn evaluate(
    allowlist: &AllowList,
    pinned: Option<Fingerprint>,
    identity: &PeerIdentity,
) -> TrustDecision {
    if !allowlist.contains(identity.peer) {
        return TrustDecision::NotAllowed;
    }
    match pinned {
        None => TrustDecision::TrustOnFirstUse,
        Some(fingerprint) if fingerprint == identity.fingerprint => TrustDecision::Trusted,
        Some(_) => TrustDecision::FingerprintMismatch,
    }
}

/// The trust authority: applies [`evaluate`] against a persisted
/// [`TrustStore`](crate::store::TrustStore) and records approvals.
#[derive(Debug)]
pub struct TrustAuthority<S> {
    store: S,
}

impl<S: TrustStore> TrustAuthority<S> {
    /// Wraps a store.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Read-only access to the underlying store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Decides whether a peer may connect, based on the current allowlist and the
    /// fingerprint pinned for it (if any). Does not mutate anything.
    pub fn authorize(&self, identity: &PeerIdentity) -> Result<TrustDecision, S::Error> {
        let allowlist = self.store.allowlist()?;
        let pinned = self.store.pinned(identity.peer)?;
        Ok(evaluate(&allowlist, pinned, identity))
    }

    /// Records a person's approval of a peer: adds it to the allowlist and pins
    /// the fingerprint it presented. This is the TOFU step — after it, the peer
    /// authorizes as [`TrustDecision::Trusted`] until its fingerprint changes.
    pub fn accept(&mut self, identity: &PeerIdentity) -> Result<(), S::Error> {
        self.store.allow(identity.peer)?;
        self.store.pin(identity.peer, identity.fingerprint)?;
        Ok(())
    }

    /// Forgets a peer entirely: removes it from the allowlist and drops its
    /// pinned fingerprint, so a future connection starts trust over from scratch.
    pub fn forget(&mut self, peer: PeerId) -> Result<(), S::Error> {
        self.store.remove_allowed(peer)?;
        self.store.unpin(peer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryTrustStore;

    const PEER: PeerId = PeerId::new(1);

    fn fp(byte: u8) -> Fingerprint {
        Fingerprint::from_bytes([byte; 32])
    }

    fn identity(byte: u8) -> PeerIdentity {
        PeerIdentity::new(PEER, fp(byte))
    }

    #[test]
    fn allowlist_tracks_membership() {
        let mut list = AllowList::new();
        assert!(list.is_empty());
        assert!(list.allow(PEER));
        assert!(!list.allow(PEER)); // already present
        assert!(list.contains(PEER));
        assert!(list.remove(PEER));
        assert!(!list.contains(PEER));
    }

    #[test]
    fn unlisted_peer_is_not_allowed() {
        let decision = evaluate(&AllowList::new(), None, &identity(1));
        assert_eq!(decision, TrustDecision::NotAllowed);
    }

    #[test]
    fn allowed_but_unseen_peer_is_trust_on_first_use() {
        let mut list = AllowList::new();
        list.allow(PEER);
        assert_eq!(
            evaluate(&list, None, &identity(1)),
            TrustDecision::TrustOnFirstUse
        );
    }

    #[test]
    fn allowed_peer_with_matching_pin_is_trusted() {
        let mut list = AllowList::new();
        list.allow(PEER);
        let decision = evaluate(&list, Some(fp(7)), &identity(7));
        assert_eq!(decision, TrustDecision::Trusted);
        assert!(decision.is_trusted());
    }

    #[test]
    fn allowed_peer_with_changed_pin_is_a_mismatch() {
        let mut list = AllowList::new();
        list.allow(PEER);
        assert_eq!(
            evaluate(&list, Some(fp(7)), &identity(8)),
            TrustDecision::FingerprintMismatch,
        );
    }

    #[test]
    fn accept_then_authorize_trusts_the_same_fingerprint() {
        let mut authority = TrustAuthority::new(InMemoryTrustStore::default());
        let id = identity(3);

        // Before acceptance the peer is unknown.
        assert_eq!(authority.authorize(&id).unwrap(), TrustDecision::NotAllowed);

        authority.accept(&id).unwrap();

        assert_eq!(authority.authorize(&id).unwrap(), TrustDecision::Trusted);
    }

    #[test]
    fn a_changed_fingerprint_after_acceptance_is_rejected() {
        let mut authority = TrustAuthority::new(InMemoryTrustStore::default());
        authority.accept(&identity(3)).unwrap();

        // Same peer, different certificate fingerprint.
        let decision = authority.authorize(&identity(9)).unwrap();

        assert_eq!(decision, TrustDecision::FingerprintMismatch);
    }

    #[test]
    fn forgetting_a_peer_resets_trust() {
        let mut authority = TrustAuthority::new(InMemoryTrustStore::default());
        let id = identity(3);
        authority.accept(&id).unwrap();

        authority.forget(PEER).unwrap();

        assert_eq!(authority.authorize(&id).unwrap(), TrustDecision::NotAllowed);
    }
}
