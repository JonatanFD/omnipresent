//! Clipboard payloads carried between machines when clipboard sharing is on.
//!
//! These live in the shared kernel because both the clipboard adapter
//! (`omni-clipboard`) and the wire [`Message`](crate::Message) need the same
//! vocabulary. The crate that talks to the OS clipboard re-exports these types,
//! so there is exactly one definition that crosses the network.

use serde::{Deserialize, Serialize};

/// The largest clipboard payload we will send or accept, in bytes. A cap keeps
/// a malicious or buggy peer from forcing a huge allocation (denial of service)
/// by announcing an enormous image or pasting a giant blob. 64 MiB is far more
/// than real text or screenshots need.
pub const MAX_CLIPBOARD_BYTES: usize = 64 * 1024 * 1024;

/// Raw image data from the clipboard, in RGBA order (8 bits per channel, so
/// four bytes per pixel).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

impl ClipboardImage {
    /// Checks that the pixel buffer is exactly the size the dimensions imply and
    /// that it is within the payload cap. The multiplication is overflow-checked
    /// so huge dimensions cannot wrap to a small expected length and slip a
    /// mismatched buffer through.
    pub fn validate(&self) -> Result<(), ClipboardValidationError> {
        let expected_len = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|len| len.checked_mul(4));
        if Some(self.bytes.len()) != expected_len {
            return Err(ClipboardValidationError(format!(
                "image byte size mismatch: expected {expected_len:?} bytes ({}x{}x4), got {}",
                self.width,
                self.height,
                self.bytes.len()
            )));
        }
        if self.bytes.len() > MAX_CLIPBOARD_BYTES {
            return Err(ClipboardValidationError(format!(
                "image is {} bytes, over the {MAX_CLIPBOARD_BYTES}-byte limit",
                self.bytes.len()
            )));
        }
        Ok(())
    }
}

/// The clipboard's contents: either text or an image. This is what travels in a
/// [`Message::Clipboard`](crate::Message::Clipboard).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardData {
    Text(String),
    Image(ClipboardImage),
}

impl ClipboardData {
    /// Rejects payloads that are malformed or too large to be worth moving
    /// across the network. Call this on both ends: before sending a local copy
    /// and before applying a remote one.
    pub fn validate(&self) -> Result<(), ClipboardValidationError> {
        match self {
            ClipboardData::Text(text) => {
                if text.len() > MAX_CLIPBOARD_BYTES {
                    return Err(ClipboardValidationError(format!(
                        "text is {} bytes, over the {MAX_CLIPBOARD_BYTES}-byte limit",
                        text.len()
                    )));
                }
                Ok(())
            }
            ClipboardData::Image(image) => image.validate(),
        }
    }
}

/// A clipboard payload was rejected (size mismatch or over the limit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardValidationError(pub String);

impl std::fmt::Display for ClipboardValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid clipboard payload: {}", self.0)
    }
}

impl std::error::Error for ClipboardValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_image_is_valid() {
        let image = ClipboardImage {
            width: 2,
            height: 2,
            bytes: vec![255; 16], // 2 * 2 * 4
        };
        assert!(image.validate().is_ok());
    }

    #[test]
    fn mismatched_image_is_rejected() {
        let image = ClipboardImage {
            width: 2,
            height: 2,
            bytes: vec![255; 15],
        };
        assert!(image.validate().is_err());
    }

    #[test]
    fn overflowing_dimensions_cannot_bypass_the_check() {
        // width * height * 4 wraps on a 32-bit usize; checked_mul yields None so
        // the buffer can never match and the payload is rejected.
        let image = ClipboardImage {
            width: u32::MAX,
            height: 4,
            bytes: vec![0; 16],
        };
        assert!(image.validate().is_err());
    }

    #[test]
    fn oversized_text_is_rejected() {
        let data = ClipboardData::Text("x".repeat(MAX_CLIPBOARD_BYTES + 1));
        assert!(data.validate().is_err());
    }
}
