//! Mapping between Windows virtual key codes (`VK_*`) and the USB HID usage
//! codes (keyboard page 0x07) that [`omni_protocol::KeyCode`] carries.
//!
//! The table covers the full ANSI layout plus navigation, function (F1–F24),
//! keypad, lock, and modifier keys. Keys without a standard HID usage are
//! deliberately absent: an unmapped key is dropped, never guessed. Left and
//! right modifiers map to their distinct HID usages, matching what the
//! low-level keyboard hook reports.

use omni_protocol::KeyCode;

/// Every (Windows virtual key, HID usage) pair we translate. One table, scanned
/// in both directions, so the two mappings can never disagree. Every virtual
/// key and every HID usage appears at most once.
const VK_HID: &[(u16, u32)] = &[
    // Letters (VK 'A'..'Z' are the ASCII codes).
    (0x41, 0x04), // A
    (0x42, 0x05), // B
    (0x43, 0x06), // C
    (0x44, 0x07), // D
    (0x45, 0x08), // E
    (0x46, 0x09), // F
    (0x47, 0x0A), // G
    (0x48, 0x0B), // H
    (0x49, 0x0C), // I
    (0x4A, 0x0D), // J
    (0x4B, 0x0E), // K
    (0x4C, 0x0F), // L
    (0x4D, 0x10), // M
    (0x4E, 0x11), // N
    (0x4F, 0x12), // O
    (0x50, 0x13), // P
    (0x51, 0x14), // Q
    (0x52, 0x15), // R
    (0x53, 0x16), // S
    (0x54, 0x17), // T
    (0x55, 0x18), // U
    (0x56, 0x19), // V
    (0x57, 0x1A), // W
    (0x58, 0x1B), // X
    (0x59, 0x1C), // Y
    (0x5A, 0x1D), // Z
    // Number row (VK '0'..'9' are the ASCII codes).
    (0x31, 0x1E), // 1
    (0x32, 0x1F), // 2
    (0x33, 0x20), // 3
    (0x34, 0x21), // 4
    (0x35, 0x22), // 5
    (0x36, 0x23), // 6
    (0x37, 0x24), // 7
    (0x38, 0x25), // 8
    (0x39, 0x26), // 9
    (0x30, 0x27), // 0
    // Controls and punctuation.
    (0x0D, 0x28), // Return (Enter)
    (0x1B, 0x29), // Escape
    (0x08, 0x2A), // Backspace
    (0x09, 0x2B), // Tab
    (0x20, 0x2C), // Space
    (0xBD, 0x2D), // - (VK_OEM_MINUS)
    (0xBB, 0x2E), // = (VK_OEM_PLUS)
    (0xDB, 0x2F), // [ (VK_OEM_4)
    (0xDD, 0x30), // ] (VK_OEM_6)
    (0xDC, 0x31), // backslash (VK_OEM_5)
    (0xBA, 0x33), // ; (VK_OEM_1)
    (0xDE, 0x34), // ' (VK_OEM_7)
    (0xC0, 0x35), // ` (VK_OEM_3)
    (0xBC, 0x36), // , (VK_OEM_COMMA)
    (0xBE, 0x37), // . (VK_OEM_PERIOD)
    (0xBF, 0x38), // / (VK_OEM_2)
    (0x14, 0x39), // Caps Lock (VK_CAPITAL)
    (0xE2, 0x64), // ISO section / non-US backslash (VK_OEM_102)
    (0x5D, 0x65), // Application / context menu (VK_APPS)
    // Function row F1–F24 (VK_F1 = 0x70 … VK_F24 = 0x87).
    (0x70, 0x3A), // F1
    (0x71, 0x3B), // F2
    (0x72, 0x3C), // F3
    (0x73, 0x3D), // F4
    (0x74, 0x3E), // F5
    (0x75, 0x3F), // F6
    (0x76, 0x40), // F7
    (0x77, 0x41), // F8
    (0x78, 0x42), // F9
    (0x79, 0x43), // F10
    (0x7A, 0x44), // F11
    (0x7B, 0x45), // F12
    (0x7C, 0x68), // F13
    (0x7D, 0x69), // F14
    (0x7E, 0x6A), // F15
    (0x7F, 0x6B), // F16
    (0x80, 0x6C), // F17
    (0x81, 0x6D), // F18
    (0x82, 0x6E), // F19
    (0x83, 0x6F), // F20
    (0x84, 0x70), // F21
    (0x85, 0x71), // F22
    (0x86, 0x72), // F23
    (0x87, 0x73), // F24
    // System keys.
    (0x2C, 0x46), // Print Screen (VK_SNAPSHOT)
    (0x91, 0x47), // Scroll Lock (VK_SCROLL)
    (0x13, 0x48), // Pause (VK_PAUSE)
    // Navigation block.
    (0x2D, 0x49), // Insert (VK_INSERT)
    (0x24, 0x4A), // Home (VK_HOME)
    (0x21, 0x4B), // Page Up (VK_PRIOR)
    (0x2E, 0x4C), // Forward Delete (VK_DELETE)
    (0x23, 0x4D), // End (VK_END)
    (0x22, 0x4E), // Page Down (VK_NEXT)
    (0x27, 0x4F), // Right Arrow (VK_RIGHT)
    (0x25, 0x50), // Left Arrow (VK_LEFT)
    (0x28, 0x51), // Down Arrow (VK_DOWN)
    (0x26, 0x52), // Up Arrow (VK_UP)
    // Keypad.
    (0x90, 0x53), // Num Lock (VK_NUMLOCK)
    (0x6F, 0x54), // Keypad / (VK_DIVIDE)
    (0x6A, 0x55), // Keypad * (VK_MULTIPLY)
    (0x6D, 0x56), // Keypad - (VK_SUBTRACT)
    (0x6B, 0x57), // Keypad + (VK_ADD)
    (0x61, 0x59), // Keypad 1 (VK_NUMPAD1)
    (0x62, 0x5A), // Keypad 2 (VK_NUMPAD2)
    (0x63, 0x5B), // Keypad 3 (VK_NUMPAD3)
    (0x64, 0x5C), // Keypad 4 (VK_NUMPAD4)
    (0x65, 0x5D), // Keypad 5 (VK_NUMPAD5)
    (0x66, 0x5E), // Keypad 6 (VK_NUMPAD6)
    (0x67, 0x5F), // Keypad 7 (VK_NUMPAD7)
    (0x68, 0x60), // Keypad 8 (VK_NUMPAD8)
    (0x69, 0x61), // Keypad 9 (VK_NUMPAD9)
    (0x60, 0x62), // Keypad 0 (VK_NUMPAD0)
    (0x6E, 0x63), // Keypad . (VK_DECIMAL)
    // Modifiers (left/right distinct, as the low-level hook reports them).
    (0xA2, 0xE0), // Left Control (VK_LCONTROL)
    (0xA0, 0xE1), // Left Shift (VK_LSHIFT)
    (0xA4, 0xE2), // Left Alt / Menu (VK_LMENU)
    (0x5B, 0xE3), // Left Windows (VK_LWIN)
    (0xA3, 0xE4), // Right Control (VK_RCONTROL)
    (0xA1, 0xE5), // Right Shift (VK_RSHIFT)
    (0xA5, 0xE6), // Right Alt / Menu (VK_RMENU)
    (0x5C, 0xE7), // Right Windows (VK_RWIN)
];

/// The HID usage for a Windows virtual key, or `None` for keys we do not carry.
pub fn hid_from_vk(vk: u16) -> Option<KeyCode> {
    VK_HID
        .iter()
        .find(|&&(v, _)| v == vk)
        .map(|&(_, hid)| KeyCode::new(hid))
}

/// The Windows virtual key for a HID usage, or `None` if it has no key here.
pub fn vk_from_hid(code: KeyCode) -> Option<u16> {
    VK_HID
        .iter()
        .find(|&&(_, hid)| hid == code.value())
        .map(|&(vk, _)| vk)
}

/// Whether a virtual key needs the `KEYEVENTF_EXTENDEDKEY` flag when injected.
/// These are the keys that share a make-code with a keypad key and are told
/// apart by the extended bit: the grey navigation/arrow cluster, the right-hand
/// modifiers, keypad divide and enter, and the Windows/menu keys.
pub fn is_extended_vk(vk: u16) -> bool {
    matches!(
        vk,
        0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28 // PgUp/PgDn/End/Home/arrows
            | 0x2D | 0x2E // Insert / Delete
            | 0xA3 | 0xA5 // Right Control / Right Alt
            | 0x5B | 0x5C | 0x5D // Left/Right Windows, Apps
            | 0x6F | 0x90 | 0x2C // Keypad /, Num Lock, Print Screen
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn table_has_no_duplicate_entries() {
        let vks: HashSet<u16> = VK_HID.iter().map(|&(vk, _)| vk).collect();
        let hids: HashSet<u32> = VK_HID.iter().map(|&(_, hid)| hid).collect();
        assert_eq!(vks.len(), VK_HID.len(), "duplicate virtual key");
        assert_eq!(hids.len(), VK_HID.len(), "duplicate HID usage");
    }

    #[test]
    fn every_mapping_round_trips() {
        for &(vk, hid) in VK_HID {
            let code = hid_from_vk(vk).expect("forward mapping");
            assert_eq!(code.value(), hid);
            assert_eq!(vk_from_hid(code), Some(vk));
        }
    }

    #[test]
    fn letters_and_space_map_to_their_hid_usages() {
        assert_eq!(hid_from_vk(0x41), Some(KeyCode::new(0x04))); // A
        assert_eq!(vk_from_hid(KeyCode::new(0x2C)), Some(0x20)); // Space
    }

    #[test]
    fn left_and_right_modifiers_are_distinct() {
        assert_eq!(hid_from_vk(0xA0), Some(KeyCode::new(0xE1))); // L Shift
        assert_eq!(hid_from_vk(0xA1), Some(KeyCode::new(0xE5))); // R Shift
    }

    #[test]
    fn unmapped_keys_are_none() {
        assert_eq!(hid_from_vk(0x07), None); // undefined VK
        assert_eq!(vk_from_hid(KeyCode::new(0xFFFF)), None);
    }

    #[test]
    fn arrows_and_right_modifiers_are_extended() {
        assert!(is_extended_vk(0x27)); // Right arrow
        assert!(is_extended_vk(0xA5)); // Right Alt
        assert!(!is_extended_vk(0x41)); // A
    }
}
