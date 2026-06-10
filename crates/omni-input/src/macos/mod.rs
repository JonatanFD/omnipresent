//! macOS adapters for the input ports, over Core Graphics.
//!
//! - [`MacosSource`] captures keyboard and mouse events with a CGEvent tap and
//!   can *suppress* them (swallow them before the OS acts) while input is
//!   routed to a remote machine.
//! - [`MacosSink`] injects events with `CGEventPost`, as if they came from
//!   real hardware.
//!
//! Both require the Accessibility permission (System Settings → Privacy &
//! Security → Accessibility) — the least privilege macOS offers for this; the
//! daemon never runs as root.

mod convert;
pub mod keymap;
mod sink;
mod source;

pub use sink::MacosSink;
pub use source::MacosSource;

/// Platform-neutral aliases the Runtime wires against.
pub type OsSource = MacosSource;
pub type OsSink = MacosSink;

use core_graphics::display::CGDisplay;
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Why a macOS input operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosInputError {
    /// The event tap could not be created. Almost always: the binary lacks the
    /// Accessibility permission.
    TapCreation,
    /// The capture thread is gone, so no more events will ever arrive.
    CaptureStopped,
    /// The OS refused to create or post an event.
    EventCreation,
}

impl std::fmt::Display for MacosInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacosInputError::TapCreation => f.write_str(
                "could not create the event tap — grant this binary the Accessibility \
                 permission (System Settings → Privacy & Security → Accessibility)",
            ),
            MacosInputError::CaptureStopped => f.write_str("input capture stopped"),
            MacosInputError::EventCreation => f.write_str("could not synthesize an OS event"),
        }
    }
}

impl std::error::Error for MacosInputError {}

/// The size of the main display in global display coordinates — the geometry
/// Topology builds the virtual desktop from.
pub fn primary_screen_size() -> Option<(u32, u32)> {
    let bounds = CGDisplay::main().bounds();
    Some((bounds.size.width as u32, bounds.size.height as u32))
}

/// Where the cursor currently is, in global display coordinates (origin at the
/// top-left of the main display).
pub fn cursor_position() -> Option<(i32, i32)> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let location = CGEvent::new(source).ok()?.location();
    Some((location.x as i32, location.y as i32))
}
