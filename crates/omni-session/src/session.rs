//! Sessions, roles, and the manager that owns their lifecycle.

use crate::events::{SessionEvent, SessionEvents};
use omni_protocol::{MachineId, SessionId};
use omni_topology::Crossing;
use std::collections::HashMap;

/// This machine's part in a session. Reversible: a session that starts with this
/// machine controlling can be flipped so the peer controls instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// This machine's keyboard and mouse are the source of input.
    Controller,
    /// This machine receives and injects the peer's input.
    Target,
}

impl Role {
    /// The opposite role.
    pub const fn reversed(self) -> Role {
        match self {
            Role::Controller => Role::Target,
            Role::Target => Role::Controller,
        }
    }
}

/// One established session with a peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub peer: MachineId,
    pub role: Role,
}

/// Where input is currently going. When this machine is the Controller the
/// cursor moves across screens; `Local` means it is on this machine's own screen,
/// `Remote` means it has crossed onto a peer that is now receiving input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTarget {
    Local,
    Remote(MachineId),
}

/// Something the caller asked of the manager that does not make sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// A session with this id already exists.
    DuplicateSession(SessionId),
    /// A session with this peer already exists.
    PeerAlreadyConnected(MachineId),
    /// No session has this id.
    UnknownSession(SessionId),
    /// The cursor crossed onto a peer we have no session with.
    NoSessionForPeer(MachineId),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::DuplicateSession(id) => write!(f, "duplicate session {}", id.value()),
            SessionError::PeerAlreadyConnected(p) => {
                write!(f, "peer {} already connected", p.value())
            }
            SessionError::UnknownSession(id) => write!(f, "unknown session {}", id.value()),
            SessionError::NoSessionForPeer(p) => write!(f, "no session for peer {}", p.value()),
        }
    }
}

impl std::error::Error for SessionError {}

/// Owns the set of active sessions, the dynamic roles, and which target is
/// currently receiving input. Emits [`SessionEvent`]s through a
/// [`SessionEvents`] sink as things change.
#[derive(Debug)]
pub struct SessionManager<E: SessionEvents> {
    local: MachineId,
    sessions: HashMap<SessionId, Session>,
    by_peer: HashMap<MachineId, SessionId>,
    active: ActiveTarget,
    events: E,
}

impl<E: SessionEvents> SessionManager<E> {
    /// Creates a manager for this machine, with input starting on the local
    /// screen and no sessions.
    pub fn new(local: MachineId, events: E) -> Self {
        Self {
            local,
            sessions: HashMap::new(),
            by_peer: HashMap::new(),
            active: ActiveTarget::Local,
            events,
        }
    }

    /// The id of this machine.
    pub fn local(&self) -> MachineId {
        self.local
    }

    /// Where input is currently going.
    pub fn active_target(&self) -> ActiveTarget {
        self.active
    }

    /// The session with the given id, if any.
    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id)
    }

    /// The session with the given peer, if any.
    pub fn session_for_peer(&self, peer: MachineId) -> Option<&Session> {
        self.by_peer.get(&peer).and_then(|id| self.sessions.get(id))
    }

    /// How many sessions are active.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether there are no active sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Read-only access to the events sink (mainly to inspect it in tests).
    pub fn events(&self) -> &E {
        &self.events
    }

    /// Establishes a new session after a connection is accepted. `role` is this
    /// machine's part: `Controller` if it initiated, `Target` if it accepted.
    pub fn establish(
        &mut self,
        id: SessionId,
        peer: MachineId,
        role: Role,
    ) -> Result<(), SessionError> {
        if self.sessions.contains_key(&id) {
            return Err(SessionError::DuplicateSession(id));
        }
        if self.by_peer.contains_key(&peer) {
            return Err(SessionError::PeerAlreadyConnected(peer));
        }
        self.sessions.insert(id, Session { id, peer, role });
        self.by_peer.insert(peer, id);
        self.events
            .emit(SessionEvent::Established { id, peer, role });
        Ok(())
    }

    /// Ends a session. If its peer was the active target, input returns to the
    /// local screen.
    pub fn close(&mut self, id: SessionId) -> Result<(), SessionError> {
        let session = self
            .sessions
            .remove(&id)
            .ok_or(SessionError::UnknownSession(id))?;
        self.by_peer.remove(&session.peer);
        if self.active == ActiveTarget::Remote(session.peer) {
            self.set_active(ActiveTarget::Local);
        }
        self.events.emit(SessionEvent::Closed { id });
        Ok(())
    }

    /// Reverses this machine's role in a session (Controller <-> Target), e.g.
    /// when the peer takes over control. Returns the new role.
    pub fn reverse_role(&mut self, id: SessionId) -> Result<Role, SessionError> {
        let session = self
            .sessions
            .get_mut(&id)
            .ok_or(SessionError::UnknownSession(id))?;
        session.role = session.role.reversed();
        let role = session.role;
        self.events.emit(SessionEvent::RoleChanged { id, role });
        Ok(role)
    }

    /// Reacts to a cursor crossing reported by Topology, switching where input is
    /// routed. Crossing back onto this machine routes input locally; crossing
    /// onto a peer routes it there (the peer must have a session).
    pub fn handle_crossing(&mut self, crossing: Crossing) -> Result<(), SessionError> {
        if crossing.peer == self.local {
            self.set_active(ActiveTarget::Local);
            return Ok(());
        }
        if !self.by_peer.contains_key(&crossing.peer) {
            return Err(SessionError::NoSessionForPeer(crossing.peer));
        }
        self.set_active(ActiveTarget::Remote(crossing.peer));
        Ok(())
    }

    /// Updates the active target, emitting an event only when it actually changes.
    fn set_active(&mut self, target: ActiveTarget) {
        if self.active != target {
            self.active = target;
            self.events
                .emit(SessionEvent::ActiveTargetChanged { target });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::RecordingEvents;
    use omni_topology::Point;

    const LOCAL: MachineId = MachineId::new(1);
    const PEER_A: MachineId = MachineId::new(2);
    const PEER_B: MachineId = MachineId::new(3);
    const S1: SessionId = SessionId::new(10);
    const S2: SessionId = SessionId::new(20);

    fn manager() -> SessionManager<RecordingEvents> {
        SessionManager::new(LOCAL, RecordingEvents::default())
    }

    fn crossing(peer: MachineId) -> Crossing {
        Crossing {
            peer,
            entry: Point::new(0, 0),
        }
    }

    #[test]
    fn starts_local_with_no_sessions() {
        let mgr = manager();
        assert!(mgr.is_empty());
        assert_eq!(mgr.active_target(), ActiveTarget::Local);
    }

    #[test]
    fn establishing_a_session_records_it_and_emits() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();

        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.session(S1).unwrap().peer, PEER_A);
        assert_eq!(mgr.session_for_peer(PEER_A).unwrap().id, S1);
        assert_eq!(
            mgr.events().events(),
            &[SessionEvent::Established {
                id: S1,
                peer: PEER_A,
                role: Role::Controller,
            }],
        );
    }

    #[test]
    fn duplicate_session_id_is_rejected() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();
        assert_eq!(
            mgr.establish(S1, PEER_B, Role::Controller),
            Err(SessionError::DuplicateSession(S1)),
        );
    }

    #[test]
    fn duplicate_peer_is_rejected() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();
        assert_eq!(
            mgr.establish(S2, PEER_A, Role::Controller),
            Err(SessionError::PeerAlreadyConnected(PEER_A)),
        );
    }

    #[test]
    fn crossing_onto_a_peer_routes_input_there() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();

        mgr.handle_crossing(crossing(PEER_A)).unwrap();

        assert_eq!(mgr.active_target(), ActiveTarget::Remote(PEER_A));
        assert_eq!(
            mgr.events().events().last(),
            Some(&SessionEvent::ActiveTargetChanged {
                target: ActiveTarget::Remote(PEER_A),
            }),
        );
    }

    #[test]
    fn crossing_back_to_local_routes_input_home() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();
        mgr.handle_crossing(crossing(PEER_A)).unwrap();

        mgr.handle_crossing(crossing(LOCAL)).unwrap();

        assert_eq!(mgr.active_target(), ActiveTarget::Local);
    }

    #[test]
    fn crossing_onto_an_unknown_peer_fails() {
        let mut mgr = manager();
        assert_eq!(
            mgr.handle_crossing(crossing(PEER_B)),
            Err(SessionError::NoSessionForPeer(PEER_B)),
        );
    }

    #[test]
    fn repeated_crossing_to_the_same_target_emits_once() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();

        mgr.handle_crossing(crossing(PEER_A)).unwrap();
        mgr.handle_crossing(crossing(PEER_A)).unwrap();

        let changes = mgr
            .events()
            .events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::ActiveTargetChanged { .. }))
            .count();
        assert_eq!(changes, 1);
    }

    #[test]
    fn closing_the_active_session_returns_input_local() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();
        mgr.handle_crossing(crossing(PEER_A)).unwrap();

        mgr.close(S1).unwrap();

        assert!(mgr.is_empty());
        assert_eq!(mgr.active_target(), ActiveTarget::Local);
        let tail = mgr.events().events();
        assert_eq!(
            &tail[tail.len() - 2..],
            &[
                SessionEvent::ActiveTargetChanged {
                    target: ActiveTarget::Local
                },
                SessionEvent::Closed { id: S1 },
            ],
        );
    }

    #[test]
    fn closing_an_unknown_session_fails() {
        let mut mgr = manager();
        assert_eq!(mgr.close(S1), Err(SessionError::UnknownSession(S1)));
    }

    #[test]
    fn reversing_a_role_flips_and_emits() {
        let mut mgr = manager();
        mgr.establish(S1, PEER_A, Role::Controller).unwrap();

        let role = mgr.reverse_role(S1).unwrap();

        assert_eq!(role, Role::Target);
        assert_eq!(mgr.session(S1).unwrap().role, Role::Target);
        assert_eq!(
            mgr.events().events().last(),
            Some(&SessionEvent::RoleChanged {
                id: S1,
                role: Role::Target,
            }),
        );
    }

    #[test]
    fn reversing_an_unknown_session_fails() {
        let mut mgr = manager();
        assert_eq!(mgr.reverse_role(S1), Err(SessionError::UnknownSession(S1)));
    }
}
