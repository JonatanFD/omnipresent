use std::fmt;

// The clipboard payload types live in the shared kernel so the wire protocol and
// this adapter agree on exactly one definition. Re-export them so the rest of
// the crate (and its callers) can keep saying `domain::ClipboardData`.
pub use omni_protocol::{ClipboardData, ClipboardImage};

/// Domain errors for clipboard operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    /// Sharing is off, so nothing may be read or written (the opt-in guard).
    Disabled,
    /// The OS clipboard backend failed.
    Platform(String),
    /// A payload was malformed or too large to accept.
    Validation(String),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardError::Disabled => {
                write!(f, "clipboard sharing is disabled (strictly opt-in)")
            }
            ClipboardError::Platform(msg) => write!(f, "platform clipboard error: {msg}"),
            ClipboardError::Validation(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// A rejected payload from the protocol layer becomes a domain validation error.
impl From<omni_protocol::ClipboardValidationError> for ClipboardError {
    fn from(err: omni_protocol::ClipboardValidationError) -> Self {
        ClipboardError::Validation(err.0)
    }
}
