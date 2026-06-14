use crate::domain::{ClipboardData, ClipboardError};
use crate::port::ClipboardPort;
use std::sync::Mutex;

/// A mock clipboard backend implementing `ClipboardPort` using an in-memory Mutex.
pub struct MockClipboardBackend {
    content: Mutex<Option<ClipboardData>>,
}

impl MockClipboardBackend {
    /// Creates a new empty mock backend.
    pub fn new() -> Self {
        Self {
            content: Mutex::new(None),
        }
    }

    /// Creates a new mock backend seeded with some initial data.
    pub fn seeded(data: ClipboardData) -> Self {
        Self {
            content: Mutex::new(Some(data)),
        }
    }

    /// Helper to directly set the mock clipboard data (simulating a user copy event).
    pub fn set_mock_data(&self, data: ClipboardData) {
        if let Ok(mut content) = self.content.lock() {
            *content = Some(data);
        }
    }

    /// Helper to directly clear the mock clipboard.
    pub fn clear_mock_data(&self) {
        if let Ok(mut content) = self.content.lock() {
            *content = None;
        }
    }

    /// Helper to directly inspect the mock clipboard data without mutating/updating the manager's sync state.
    pub fn get_mock_data(&self) -> Option<ClipboardData> {
        self.content.lock().ok().and_then(|guard| guard.clone())
    }
}

impl Default for MockClipboardBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardPort for MockClipboardBackend {
    fn read(&self) -> Result<Option<ClipboardData>, ClipboardError> {
        let content = self
            .content
            .lock()
            .map_err(|_| ClipboardError::Platform("Lock poisoned".to_string()))?;
        Ok(content.clone())
    }

    fn write(&self, data: &ClipboardData) -> Result<(), ClipboardError> {
        let mut content = self
            .content
            .lock()
            .map_err(|_| ClipboardError::Platform("Lock poisoned".to_string()))?;
        *content = Some(data.clone());
        Ok(())
    }
}
