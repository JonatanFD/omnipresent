//! Input: the only module allowed to touch the OS input subsystem.
//!
//! Captures local keyboard and mouse events and injects remote events into the
//! local OS when this machine is the Target. Platform specifics live behind the
//! [`InputSource`] and [`InputSink`] ports; the domain never sees a platform type.
//!
//! The crate provides the ports, in-memory adapters for tests, and the real
//! per-OS adapters: macOS (CGEvent tap + CGEventPost) and Linux (evdev +
//! uinput). The platform module for the current OS is re-exported as
//! [`platform`], with its source/sink under the common names `OsSource` and
//! `OsSink`.

pub mod diag;
pub mod memory;
pub mod port;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux as platform;
#[cfg(target_os = "macos")]
pub use macos as platform;

pub use memory::{QueuedSource, RecordingSink};
pub use port::{InputSink, InputSource};
