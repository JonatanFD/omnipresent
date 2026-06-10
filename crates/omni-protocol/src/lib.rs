//! Shared kernel: the common vocabulary every other module speaks.
//!
//! Holds the value objects and wire messages exchanged between machines —
//! input events, control messages, shared identifiers, and the binary
//! (de)serialization format. This crate is the leaf of the dependency graph: it
//! depends on nothing internal, and everything else depends on it.

pub mod control;
pub mod ids;
pub mod input;
pub mod wire;

// Flattened re-exports so callers write `omni_protocol::InputEvent` rather than
// reaching into each submodule.
pub use control::{ControlMessage, RejectReason, ScreenSize};
pub use ids::{Fingerprint, MachineId, PeerId, SessionId};
pub use input::{Action, InputEvent, KeyCode, Modifiers, MouseButton, MouseDelta, ScrollDelta};
pub use wire::{CodecError, Message, decode, encode};
