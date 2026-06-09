//! The `SessionEvents` port: how Session tells the outside world what changed.
//!
//! The manager never talks to the CLI or the network directly. It emits these
//! events and the Runtime reacts — notifying the user, sending control messages,
//! switching where input is routed.

use crate::session::{ActiveTarget, Role};
use omni_protocol::{MachineId, SessionId};

/// Something that happened to a session or to where input is going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    /// A new session was established with `peer`, with this machine in `role`.
    Established {
        id: SessionId,
        peer: MachineId,
        role: Role,
    },
    /// A session ended (disconnect or loss).
    Closed { id: SessionId },
    /// This machine's role in a session was reversed.
    RoleChanged { id: SessionId, role: Role },
    /// Input is now flowing to a different place (a peer, or back to local).
    ActiveTargetChanged { target: ActiveTarget },
}

/// Receives session events. Real adapters forward them to the Runtime; the
/// in-crate [`RecordingEvents`] collects them for tests.
pub trait SessionEvents {
    fn emit(&mut self, event: SessionEvent);
}

/// A `SessionEvents` that records everything it receives, in order.
#[derive(Debug, Default)]
pub struct RecordingEvents {
    events: Vec<SessionEvent>,
}

impl RecordingEvents {
    /// The events received so far, oldest first.
    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }
}

impl SessionEvents for RecordingEvents {
    fn emit(&mut self, event: SessionEvent) {
        self.events.push(event);
    }
}
