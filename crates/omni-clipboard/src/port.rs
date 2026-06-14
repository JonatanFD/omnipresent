use crate::domain::{ClipboardData, ClipboardError};

/// Port interface for reading and writing to the system clipboard.
pub trait ClipboardPort: Send + Sync {
    /// Reads the current content of the system clipboard if it's text or a supported image.
    fn read(&self) -> Result<Option<ClipboardData>, ClipboardError>;

    /// Overwrites the system clipboard with the provided data payload.
    fn write(&self, data: &ClipboardData) -> Result<(), ClipboardError>;
}
