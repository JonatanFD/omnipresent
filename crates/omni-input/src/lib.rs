//! Input: the only module allowed to touch the OS input subsystem.
//!
//! Captures local keyboard and mouse events and injects remote events into the
//! local OS when this machine is the Target. Platform specifics live behind the
//! `InputSource` and `InputSink` ports, implemented by per-OS adapters (macOS
//! CGEvent/IOKit, Linux evdev/uinput).
