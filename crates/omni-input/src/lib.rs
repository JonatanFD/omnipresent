//! Input: the only module allowed to touch the OS input subsystem.
//!
//! Captures local keyboard and mouse events and injects remote events into the
//! local OS when this machine is the Target. Platform specifics live behind the
//! [`InputSource`] and [`InputSink`] ports; the domain never sees a platform type.
//!
//! Today this crate provides the ports and in-memory adapters used to drive and
//! test the pipelines. The real per-OS adapters — macOS (CGEvent/IOKit) and Linux
//! (evdev/uinput) — are a planned follow-up; they need platform APIs, the
//! least-privilege model from `CLAUDE.md`, and live hardware to exercise.

pub mod memory;
pub mod port;

pub use memory::{QueuedSource, RecordingSink};
pub use port::{InputSink, InputSource};
