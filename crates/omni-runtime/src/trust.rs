//! The daemon's trust state: Security's allowlist/TOFU policy persisted to a
//! JSON file, plus the host pins (known-hosts style) used when dialing out.
//!
//! Identity model:
//! - A peer **is** its certificate fingerprint. The `PeerId` Security keys
//!   trust on is derived from the fingerprint's first eight bytes.
//! - Inbound, the allowlist gates everything: an unknown fingerprint may
//!   complete the TLS handshake only so its `ConnectRequest` can be shown to
//!   the user for `omni accept` / `omni reject` — no input is processed and no
//!   session exists until the user (or a previous pin) trusts it.
//! - Outbound, TOFU pins the fingerprint a *host* presented on first connect;
//!   any later change is refused inside the TLS handshake.

use omni_protocol::{Fingerprint, PeerId};
use omni_security::{PeerIdentity, TrustDecision, TrustStore, evaluate};
use omni_transport::{HandshakePolicy, PolicyViolation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// One remembered peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRecord {
    /// The host this peer was dialed at, if we ever dialed it (the TOFU key
    /// for outbound connections).
    pub host: Option<String>,
    /// Lowercase hex SHA-256 of the peer's certificate.
    pub fingerprint: String,
}

/// The persisted shape of the trust file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct TrustFile {
    peers: Vec<PeerRecord>,
}

/// The `PeerId` Security keys a fingerprint's trust on.
pub fn peer_id_of(fingerprint: Fingerprint) -> PeerId {
    let bytes = fingerprint.as_bytes();
    let mut head = [0u8; 8];
    head.copy_from_slice(&bytes[..8]);
    PeerId::new(u64::from_be_bytes(head))
}

/// Renders a fingerprint the way the trust file stores it.
pub fn fingerprint_hex(fingerprint: Fingerprint) -> String {
    fingerprint.to_string()
}

/// Collapses duplicate host pins an earlier `accept` could leave behind: a host
/// maps to a single certificate, so only the most recently accepted record for
/// each host is kept (records were always appended, so the last one wins).
/// Host-less pins are never duplicates by host and are all kept. Returns whether
/// anything was dropped.
fn dedup_host_pins(peers: &mut Vec<PeerRecord>) -> bool {
    let before = peers.len();
    let mut seen = std::collections::HashSet::new();
    // Walk newest-first so the last record for a host wins, then restore order.
    let mut kept: Vec<PeerRecord> = peers
        .drain(..)
        .rev()
        .filter(|r| match &r.host {
            Some(host) => seen.insert(host.clone()),
            None => true,
        })
        .collect();
    kept.reverse();
    *peers = kept;
    peers.len() != before
}

fn parse_fingerprint(hex: &str) -> Option<Fingerprint> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        bytes[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(Fingerprint::from_bytes(bytes))
}

/// Why reading or writing the trust file failed.
#[derive(Debug)]
pub enum TrustError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for TrustError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustError::Io(e) => write!(f, "trust store i/o failed: {e}"),
            TrustError::Parse(e) => write!(f, "trust store file is invalid: {e}"),
        }
    }
}

impl std::error::Error for TrustError {}

/// The daemon's live trust state. Shared (`Arc`) between the QUIC handshake
/// verifiers, the connection handlers, and the IPC commands.
#[derive(Debug)]
pub struct TrustState {
    path: PathBuf,
    inner: Mutex<TrustFile>,
}

impl TrustState {
    /// Loads the trust file, or starts empty if it does not exist. Collapses any
    /// duplicate host pins an earlier version could have left behind, and
    /// rewrites the file if it changed, so a stale pin can no longer shadow the
    /// current one.
    pub fn load(path: PathBuf) -> Result<Self, TrustError> {
        let mut inner: TrustFile = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(TrustError::Parse)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => TrustFile::default(),
            Err(e) => return Err(TrustError::Io(e)),
        };
        let state = Self {
            path,
            inner: Mutex::new(TrustFile::default()),
        };
        if dedup_host_pins(&mut inner.peers) {
            state.save(&inner)?;
        }
        *state.inner.lock().expect("trust state lock") = inner;
        Ok(state)
    }

    fn save(&self, file: &TrustFile) -> Result<(), TrustError> {
        let bytes = serde_json::to_vec_pretty(file).map_err(TrustError::Parse)?;
        std::fs::write(&self.path, bytes).map_err(TrustError::Io)
    }

    /// Security's verdict on an inbound peer presenting `fingerprint`.
    pub fn decide_inbound(&self, fingerprint: Fingerprint) -> TrustDecision {
        let file = self.inner.lock().expect("trust state lock");
        let store = MemStore(&file);
        let peer = peer_id_of(fingerprint);
        evaluate(
            &store.allowlist().unwrap_or_default(),
            store.pinned(peer).unwrap_or(None),
            &PeerIdentity::new(peer, fingerprint),
        )
    }

    /// Whether this exact fingerprint has been accepted before.
    pub fn is_trusted(&self, fingerprint: Fingerprint) -> bool {
        self.decide_inbound(fingerprint).is_trusted()
    }

    /// Records the user's acceptance of a peer (TOFU pin + allowlist), with
    /// the host we know it by, if any.
    ///
    /// A host pins exactly one certificate. When a host is given, any other
    /// record that claimed it with a different fingerprint is dropped, so a
    /// rotated certificate the user just accepted replaces the old pin instead
    /// of being shadowed by it (a stale pin would otherwise refuse the peer the
    /// next time we dialed that host).
    pub fn accept(&self, fingerprint: Fingerprint, host: Option<&str>) -> Result<(), TrustError> {
        let mut file = self.inner.lock().expect("trust state lock");
        let hex = fingerprint_hex(fingerprint);
        if let Some(host) = host {
            file.peers
                .retain(|r| r.fingerprint == hex || r.host.as_deref() != Some(host));
        }
        if let Some(record) = file.peers.iter_mut().find(|r| r.fingerprint == hex) {
            if let Some(host) = host {
                record.host = Some(host.to_string());
            }
        } else {
            file.peers.push(PeerRecord {
                host: host.map(str::to_string),
                fingerprint: hex,
            });
        }
        self.save(&file)
    }

    /// Forgets a peer by host or fingerprint prefix. Returns whether anything
    /// was removed.
    pub fn remove(&self, selector: &str) -> Result<bool, TrustError> {
        let mut file = self.inner.lock().expect("trust state lock");
        let before = file.peers.len();
        file.peers.retain(|r| {
            r.host.as_deref() != Some(selector) && !r.fingerprint.starts_with(selector)
        });
        let removed = file.peers.len() != before;
        if removed {
            self.save(&file)?;
        }
        Ok(removed)
    }

    /// Every remembered peer.
    pub fn peers(&self) -> Vec<PeerRecord> {
        self.inner.lock().expect("trust state lock").peers.clone()
    }

    /// The fingerprint pinned for a host we dial, if any.
    pub fn pinned_for_host(&self, host: &str) -> Option<Fingerprint> {
        let file = self.inner.lock().expect("trust state lock");
        file.peers
            .iter()
            .find(|r| r.host.as_deref() == Some(host))
            .and_then(|r| parse_fingerprint(&r.fingerprint))
    }
}

/// A read-only `TrustStore` view over the file contents, so the decision goes
/// through Security's `evaluate` rather than re-implementing the policy here.
struct MemStore<'a>(&'a TrustFile);

impl TrustStore for MemStore<'_> {
    type Error = std::convert::Infallible;

    fn allowlist(&self) -> Result<omni_security::AllowList, Self::Error> {
        let mut list = omni_security::AllowList::new();
        for record in &self.0.peers {
            if let Some(fp) = parse_fingerprint(&record.fingerprint) {
                list.allow(peer_id_of(fp));
            }
        }
        Ok(list)
    }

    fn pinned(&self, peer: PeerId) -> Result<Option<Fingerprint>, Self::Error> {
        Ok(self
            .0
            .peers
            .iter()
            .filter_map(|r| parse_fingerprint(&r.fingerprint))
            .find(|fp| peer_id_of(*fp) == peer))
    }

    fn allow(&mut self, _peer: PeerId) -> Result<(), Self::Error> {
        unreachable!("read-only view")
    }

    fn remove_allowed(&mut self, _peer: PeerId) -> Result<(), Self::Error> {
        unreachable!("read-only view")
    }

    fn pin(&mut self, _peer: PeerId, _fp: Fingerprint) -> Result<(), Self::Error> {
        unreachable!("read-only view")
    }

    fn unpin(&mut self, _peer: PeerId) -> Result<(), Self::Error> {
        unreachable!("read-only view")
    }
}

/// The handshake policy Transport enforces inside its TLS verifiers.
impl HandshakePolicy for TrustState {
    fn authorize_server(
        &self,
        host: &str,
        fingerprint: Fingerprint,
    ) -> Result<(), PolicyViolation> {
        match self.pinned_for_host(host) {
            // TOFU: a host that changes its certificate is refused hard.
            Some(pinned) if pinned != fingerprint => Err(PolicyViolation::new(
                "the server's certificate fingerprint changed since it was pinned",
            )),
            _ => Ok(()),
        }
    }

    fn authorize_client(&self, _fingerprint: Fingerprint) -> Result<(), PolicyViolation> {
        // Unknown peers may finish the handshake so their connect request can
        // be shown to the user; they get no session and no input processing
        // until accepted (see `decide_inbound`).
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(byte: u8) -> Fingerprint {
        Fingerprint::from_bytes([byte; 32])
    }

    fn temp_state(name: &str) -> TrustState {
        let dir =
            std::env::temp_dir().join(format!("omni-test-trust-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TrustState::load(dir.join("trust.json")).unwrap()
    }

    #[test]
    fn unknown_peers_are_not_allowed_inbound() {
        let state = temp_state("unknown");
        assert_eq!(state.decide_inbound(fp(1)), TrustDecision::NotAllowed);
        assert!(!state.is_trusted(fp(1)));
    }

    #[test]
    fn accepted_peers_become_trusted_and_persist() {
        let state = temp_state("accept");
        state.accept(fp(2), Some("10.0.0.2")).unwrap();

        assert!(state.is_trusted(fp(2)));
        assert_eq!(state.pinned_for_host("10.0.0.2"), Some(fp(2)));

        // A fresh load sees the same state.
        let reloaded = TrustState::load(state.path.clone()).unwrap();
        assert!(reloaded.is_trusted(fp(2)));
    }

    #[test]
    fn outbound_tofu_refuses_a_changed_host_fingerprint() {
        let state = temp_state("tofu");
        state.accept(fp(3), Some("10.0.0.3")).unwrap();

        assert!(state.authorize_server("10.0.0.3", fp(3)).is_ok());
        assert!(state.authorize_server("10.0.0.3", fp(4)).is_err());
        // A host we never pinned is first-use: allowed, pinned after success.
        assert!(state.authorize_server("10.0.0.9", fp(9)).is_ok());
    }

    #[test]
    fn re_accepting_a_host_with_a_new_fingerprint_replaces_the_old_pin() {
        let state = temp_state("rotate");
        // First the host presents one certificate, then a different one (e.g.
        // the peer was reinstalled). Accepting the new one must replace the pin.
        state.accept(fp(10), Some("192.168.0.118")).unwrap();
        state.accept(fp(11), Some("192.168.0.118")).unwrap();

        // Exactly one pin for the host, and it is the new fingerprint — so a
        // later dial of that host authorizes the new certificate, not the old.
        assert_eq!(state.pinned_for_host("192.168.0.118"), Some(fp(11)));
        assert!(state.authorize_server("192.168.0.118", fp(11)).is_ok());
        assert!(state.authorize_server("192.168.0.118", fp(10)).is_err());
        // The old certificate is no longer trusted at all.
        assert!(!state.is_trusted(fp(10)));
        assert!(state.is_trusted(fp(11)));
        assert_eq!(state.peers().len(), 1);
    }

    #[test]
    fn loading_collapses_duplicate_host_pins_keeping_the_newest() {
        // A trust file an older buggy version could have written: two records
        // for the same host, the stale one first. Loading must heal it so the
        // stale pin no longer shadows the current one.
        let dir = std::env::temp_dir().join(format!("omni-test-trust-dup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trust.json");
        let file = TrustFile {
            peers: vec![
                PeerRecord {
                    host: Some("192.168.0.118".into()),
                    fingerprint: fingerprint_hex(fp(20)),
                },
                PeerRecord {
                    host: Some("192.168.0.118".into()),
                    fingerprint: fingerprint_hex(fp(21)),
                },
            ],
        };
        std::fs::write(&path, serde_json::to_vec_pretty(&file).unwrap()).unwrap();

        let state = TrustState::load(path.clone()).unwrap();

        assert_eq!(state.peers().len(), 1);
        assert_eq!(state.pinned_for_host("192.168.0.118"), Some(fp(21)));
        // The heal is persisted, so a fresh load sees the collapsed file too.
        let reloaded = TrustState::load(path).unwrap();
        assert_eq!(reloaded.peers().len(), 1);
    }

    #[test]
    fn removing_a_peer_resets_trust() {
        let state = temp_state("remove");
        state.accept(fp(5), Some("10.0.0.5")).unwrap();

        assert!(state.remove("10.0.0.5").unwrap());
        assert!(!state.is_trusted(fp(5)));
        assert!(!state.remove("10.0.0.5").unwrap());
    }

    #[test]
    fn peers_can_be_removed_by_fingerprint_prefix() {
        let state = temp_state("prefix");
        state.accept(fp(6), None).unwrap();
        let prefix = &fingerprint_hex(fp(6))[..12];

        assert!(state.remove(prefix).unwrap());
        assert!(state.peers().is_empty());
    }
}
