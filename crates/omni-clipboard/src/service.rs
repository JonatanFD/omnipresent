use crate::domain::{ClipboardData, ClipboardError};
use crate::port::ClipboardPort;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Application service that coordinates clipboard operations and enforces invariants.
pub struct ClipboardManager<P: ClipboardPort> {
    port: P,
    enabled: AtomicBool,
    last_state: Mutex<ClipboardState>,
}

#[derive(Default)]
struct ClipboardState {
    /// The last clipboard content successfully read or written to prevent loops.
    last_synced: Option<ClipboardData>,
}

impl<P: ClipboardPort> ClipboardManager<P> {
    /// Creates a new manager. `enabled` dictates the initial opt-in status.
    pub fn new(port: P, enabled: bool) -> Self {
        Self {
            port,
            enabled: AtomicBool::new(enabled),
            last_state: Mutex::new(ClipboardState::default()),
        }
    }

    /// Dynamically toggles clipboard sharing at runtime.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        if !enabled && let Ok(mut state) = self.last_state.lock() {
            // Forget history so re-enabling re-syncs from a clean slate.
            state.last_synced = None;
        }
    }

    /// Checks if the local clipboard has changed.
    /// Returns `Some(ClipboardData)` if a new local copy event is detected.
    /// Returns `None` if no change has occurred, or if sharing is disabled.
    pub fn poll_local_change(&self) -> Result<Option<ClipboardData>, ClipboardError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(ClipboardError::Disabled);
        }

        let current = self.port.read()?;

        let mut state = self
            .last_state
            .lock()
            .map_err(|_| ClipboardError::Platform("Lock poisoned".to_string()))?;

        match (&current, &state.last_synced) {
            (Some(cur_data), Some(last_data)) if cur_data == last_data => {
                // Ignore matching payloads (prevent feedback loop / redundant transmissions)
                Ok(None)
            }
            (Some(cur_data), _) => {
                // Record it either way, so a payload we choose not to send is not
                // re-examined on every poll.
                state.last_synced = Some(cur_data.clone());
                // Don't propagate anything malformed or over the size cap.
                if let Err(e) = cur_data.validate() {
                    tracing::warn!(%e, "skipping local clipboard payload");
                    return Ok(None);
                }
                Ok(Some(cur_data.clone()))
            }
            (None, _) => {
                // Clipboard is empty or format unsupported
                Ok(None)
            }
        }
    }

    /// Injects a remote clipboard update into the local OS clipboard.
    /// Updates the internal state to prevent echoing this write back.
    pub fn handle_remote_update(&self, data: ClipboardData) -> Result<(), ClipboardError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(ClipboardError::Disabled);
        }

        // Reject malformed or oversized payloads before touching the OS clipboard.
        data.validate()?;

        // Write to system clipboard
        self.port.write(&data)?;

        // Update last synced to match what we just wrote
        let mut state = self
            .last_state
            .lock()
            .map_err(|_| ClipboardError::Platform("Lock poisoned".to_string()))?;
        state.last_synced = Some(data);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ClipboardData, ClipboardError, ClipboardImage};
    use crate::memory::MockClipboardBackend;

    // Test Case 1: Serialization Roundtrip
    #[test]
    fn test_clipboard_data_serialization() {
        let text_data = ClipboardData::Text("Hello World".to_string());
        let encoded_text = postcard::to_allocvec(&text_data).expect("serialize text");
        let decoded_text: ClipboardData =
            postcard::from_bytes(&encoded_text).expect("deserialize text");
        assert_eq!(text_data, decoded_text);

        let image_data = ClipboardData::Image(ClipboardImage {
            width: 2,
            height: 2,
            bytes: vec![255; 16], // 2x2x4 = 16 bytes
        });
        let encoded_image = postcard::to_allocvec(&image_data).expect("serialize image");
        let decoded_image: ClipboardData =
            postcard::from_bytes(&encoded_image).expect("deserialize image");
        assert_eq!(image_data, decoded_image);
    }

    // Test Case 2: Opt-In Enforcement (Disabled state)
    #[test]
    fn test_manager_enforces_opt_in() {
        let mock = MockClipboardBackend::seeded(ClipboardData::Text("Initial content".to_string()));
        let manager = ClipboardManager::new(mock, false);

        // poll_local_change should return Err(ClipboardError::Disabled)
        let res = manager.poll_local_change();
        assert_eq!(res, Err(ClipboardError::Disabled));

        // handle_remote_update should return Err(ClipboardError::Disabled)
        let res = manager.handle_remote_update(ClipboardData::Text("Remote update".to_string()));
        assert_eq!(res, Err(ClipboardError::Disabled));

        // Verify the mock clipboard content remains unchanged
        assert_eq!(
            manager.port.get_mock_data(),
            Some(ClipboardData::Text("Initial content".to_string()))
        );
    }

    // Test Case 3: Local Change Detection
    #[test]
    fn test_manager_detects_local_change() {
        let mock = MockClipboardBackend::new();
        let manager = ClipboardManager::new(mock, true);

        // Initially empty clipboard -> returns Ok(None)
        assert_eq!(manager.poll_local_change(), Ok(None));

        // User copies text
        let copied = ClipboardData::Text("Changed!".to_string());
        manager.port.set_mock_data(copied.clone());

        // First poll -> detects change
        assert_eq!(manager.poll_local_change(), Ok(Some(copied)));

        // Subsequent poll -> returns Ok(None) because it's already synced
        assert_eq!(manager.poll_local_change(), Ok(None));
    }

    // Test Case 4: Echo Protection (Loop Prevention)
    #[test]
    fn test_manager_prevents_feedback_loop() {
        let mock = MockClipboardBackend::new();
        let manager = ClipboardManager::new(mock, true);

        // Remote update comes in
        let remote_update = ClipboardData::Text("Synced".to_string());
        let res = manager.handle_remote_update(remote_update.clone());
        assert_eq!(res, Ok(()));

        // Verify the OS clipboard contains "Synced"
        assert_eq!(manager.port.get_mock_data(), Some(remote_update));

        // Polling now should return Ok(None) to prevent feedback loop
        assert_eq!(manager.poll_local_change(), Ok(None));
    }

    // Test Case 5: Image Size Validation
    #[test]
    fn test_image_dimension_validation() {
        let invalid_image = ClipboardImage {
            width: 2,
            height: 2,
            bytes: vec![255; 15], // Expected 16, got 15
        };
        assert!(invalid_image.validate().is_err());

        let valid_image = ClipboardImage {
            width: 2,
            height: 2,
            bytes: vec![255; 16],
        };
        assert!(valid_image.validate().is_ok());
    }

    // Test Case 6: Image Size Validation Overflow
    #[test]
    fn test_image_dimension_validation_overflow() {
        let invalid_image = ClipboardImage {
            width: u32::MAX,
            height: 4,
            bytes: vec![255; 16], // Expected overflow, should err
        };
        assert!(invalid_image.validate().is_err());
    }

    // Test Case 7: Dynamic Enabling Toggling
    #[test]
    fn test_manager_set_enabled_dynamic() {
        let mock = MockClipboardBackend::seeded(ClipboardData::Text("Content".to_string()));
        let manager = ClipboardManager::new(mock, false);

        // Initially disabled
        assert_eq!(manager.poll_local_change(), Err(ClipboardError::Disabled));

        // Enable dynamically using &self
        manager.set_enabled(true);

        // Should now poll successfully (detects first change)
        assert!(manager.poll_local_change().is_ok());

        // Disable dynamically
        manager.set_enabled(false);
        assert_eq!(manager.poll_local_change(), Err(ClipboardError::Disabled));
    }
}
