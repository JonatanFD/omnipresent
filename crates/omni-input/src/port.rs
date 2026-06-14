//! The ports through which the rest of the system reaches the OS input
//! subsystem. Domain code depends only on these traits; the platform-specific
//! code that actually talks to the OS lives in adapters behind them.

use omni_protocol::InputEvent;

/// Captures input events from the local OS — the only way the system reads the
/// keyboard and mouse. Implemented per platform (macOS, Linux).
pub trait InputSource {
    /// What can go wrong reading from the OS.
    type Error;

    /// Returns the next captured event, or `None` if none is available right now.
    /// Non-blocking: the event loop polls it repeatedly.
    fn poll(&mut self) -> Result<Option<InputEvent>, Self::Error>;

    /// Tells the source whether its events are currently routed to a remote
    /// machine. While suppressed, captured events keep flowing through `poll`
    /// but are withheld from the local OS, so input never acts on two machines
    /// at once. Sources with nothing to withhold (like the in-memory test
    /// source) ignore this.
    fn set_suppressed(&mut self, _suppressed: bool) {}
}

/// Injects input events into the local OS, used when this machine is the Target
/// and is receiving a peer's keyboard and mouse. Implemented per platform.
pub trait InputSink {
    /// What can go wrong writing to the OS.
    type Error;

    /// Synthesizes one event into the local OS as if it came from real hardware.
    fn inject(&mut self, event: InputEvent) -> Result<(), Self::Error>;

    /// Moves the local cursor to an absolute screen position, used when the
    /// cursor enters this machine on an edge crossing. Sinks that cannot
    /// position absolutely approximate or ignore it.
    fn warp(&mut self, _x: i32, _y: i32) -> Result<(), Self::Error> {
        Ok(())
    }
}
