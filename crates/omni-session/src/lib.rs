//! Session: owns the lifecycle and role assignment of active sessions.
//!
//! Establishes a session when a connection is accepted and tears it down on
//! disconnect or loss. Assigns and flips the Controller/Target roles
//! dynamically, and tracks which target is currently receiving input — switching
//! it in response to Topology edge crossings. Everything it does is reported
//! through the `SessionEvents` port for the Runtime to act on.

pub mod events;
pub mod session;

pub use events::{RecordingEvents, SessionEvent, SessionEvents};
pub use session::{ActiveTarget, Role, Session, SessionError, SessionManager};
