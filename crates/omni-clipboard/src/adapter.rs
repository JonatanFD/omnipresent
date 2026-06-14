use crate::domain::{ClipboardData, ClipboardError, ClipboardImage};
use crate::port::ClipboardPort;
use arboard::Clipboard;
use std::borrow::Cow;

/// Production adapter implementing `ClipboardPort` using the `arboard` crate.
pub struct ArboardAdapter;

impl ArboardAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ArboardAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardPort for ArboardAdapter {
    fn read(&self) -> Result<Option<ClipboardData>, ClipboardError> {
        let mut clipboard =
            Clipboard::new().map_err(|e| ClipboardError::Platform(e.to_string()))?;

        // 1. Try reading text
        if let Ok(text) = clipboard.get_text()
            && !text.is_empty()
        {
            return Ok(Some(ClipboardData::Text(text)));
        }

        // 2. Try reading image
        if let Ok(image) = clipboard.get_image() {
            return Ok(Some(ClipboardData::Image(ClipboardImage {
                width: image.width as u32,
                height: image.height as u32,
                bytes: image.bytes.into_owned(),
            })));
        }

        Ok(None)
    }

    fn write(&self, data: &ClipboardData) -> Result<(), ClipboardError> {
        let mut clipboard =
            Clipboard::new().map_err(|e| ClipboardError::Platform(e.to_string()))?;

        match data {
            ClipboardData::Text(text) => {
                clipboard
                    .set_text(text.clone())
                    .map_err(|e| ClipboardError::Platform(e.to_string()))?;
            }
            ClipboardData::Image(image) => {
                image.validate()?;
                let img_data = arboard::ImageData {
                    width: image.width as usize,
                    height: image.height as usize,
                    bytes: Cow::Borrowed(&image.bytes),
                };
                clipboard
                    .set_image(img_data)
                    .map_err(|e| ClipboardError::Platform(e.to_string()))?;
            }
        }
        Ok(())
    }
}
