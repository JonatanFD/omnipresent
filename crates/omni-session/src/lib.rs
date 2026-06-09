//! Session: owns the lifecycle and role assignment of active sessions.
//!
//! Establishes a session when a connection is accepted and tears it down on
//! disconnect or loss. Assigns and flips the Controller/Target roles
//! dynamically, driven by Topology edge crossings and explicit connect/disconnect,
//! and tracks which Target is currently receiving input.
