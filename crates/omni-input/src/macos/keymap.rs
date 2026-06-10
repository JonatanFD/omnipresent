//! Mapping between macOS virtual key codes (Carbon `kVK_*`) and the USB HID
//! usage codes (keyboard page 0x07) that [`omni_protocol::KeyCode`] carries.
//!
//! The table covers the full ANSI layout plus navigation, function, keypad,
//! and modifier keys. Keys without a standard HID usage (media keys, the `fn`
//! key) are deliberately absent: an unmapped key is dropped, never guessed.

use omni_protocol::KeyCode;

/// Every (macOS virtual key, HID usage) pair we translate. One table, scanned
/// in both directions, so the two mappings can never disagree.
const VK_HID: &[(u16, u32)] = &[
    // Letters.
    (0, 0x04),  // A
    (11, 0x05), // B
    (8, 0x06),  // C
    (2, 0x07),  // D
    (14, 0x08), // E
    (3, 0x09),  // F
    (5, 0x0A),  // G
    (4, 0x0B),  // H
    (34, 0x0C), // I
    (38, 0x0D), // J
    (40, 0x0E), // K
    (37, 0x0F), // L
    (46, 0x10), // M
    (45, 0x11), // N
    (31, 0x12), // O
    (35, 0x13), // P
    (12, 0x14), // Q
    (15, 0x15), // R
    (1, 0x16),  // S
    (17, 0x17), // T
    (32, 0x18), // U
    (9, 0x19),  // V
    (13, 0x1A), // W
    (7, 0x1B),  // X
    (16, 0x1C), // Y
    (6, 0x1D),  // Z
    // Number row.
    (18, 0x1E), // 1
    (19, 0x1F), // 2
    (20, 0x20), // 3
    (21, 0x21), // 4
    (23, 0x22), // 5
    (22, 0x23), // 6
    (26, 0x24), // 7
    (28, 0x25), // 8
    (25, 0x26), // 9
    (29, 0x27), // 0
    // Controls and punctuation.
    (36, 0x28),  // Return
    (53, 0x29),  // Escape
    (51, 0x2A),  // Delete (backspace)
    (48, 0x2B),  // Tab
    (49, 0x2C),  // Space
    (27, 0x2D),  // -
    (24, 0x2E),  // =
    (33, 0x2F),  // [
    (30, 0x30),  // ]
    (42, 0x31),  // backslash
    (41, 0x33),  // ;
    (39, 0x34),  // '
    (50, 0x35),  // `
    (43, 0x36),  // ,
    (47, 0x37),  // .
    (44, 0x38),  // /
    (57, 0x39),  // Caps Lock
    (10, 0x64),  // ISO section (non-US backslash)
    (110, 0x65), // Application / context menu (PC keyboards)
    // Function row.
    (122, 0x3A), // F1
    (120, 0x3B), // F2
    (99, 0x3C),  // F3
    (118, 0x3D), // F4
    (96, 0x3E),  // F5
    (97, 0x3F),  // F6
    (98, 0x40),  // F7
    (100, 0x41), // F8
    (101, 0x42), // F9
    (109, 0x43), // F10
    (103, 0x44), // F11
    (111, 0x45), // F12
    (105, 0x68), // F13
    (107, 0x69), // F14
    (113, 0x6A), // F15
    (106, 0x6B), // F16
    (64, 0x6C),  // F17
    (79, 0x6D),  // F18
    (80, 0x6E),  // F19
    (90, 0x6F),  // F20
    // Navigation block.
    (114, 0x49), // Help (maps to HID Insert, its physical slot on PC boards)
    (115, 0x4A), // Home
    (116, 0x4B), // Page Up
    (117, 0x4C), // Forward Delete
    (119, 0x4D), // End
    (121, 0x4E), // Page Down
    (124, 0x4F), // Right Arrow
    (123, 0x50), // Left Arrow
    (125, 0x51), // Down Arrow
    (126, 0x52), // Up Arrow
    // Keypad.
    (71, 0x53), // Clear (HID Num Lock / Clear)
    (75, 0x54), // Keypad /
    (67, 0x55), // Keypad *
    (78, 0x56), // Keypad -
    (69, 0x57), // Keypad +
    (76, 0x58), // Keypad Enter
    (83, 0x59), // Keypad 1
    (84, 0x5A), // Keypad 2
    (85, 0x5B), // Keypad 3
    (86, 0x5C), // Keypad 4
    (87, 0x5D), // Keypad 5
    (88, 0x5E), // Keypad 6
    (89, 0x5F), // Keypad 7
    (91, 0x60), // Keypad 8
    (92, 0x61), // Keypad 9
    (82, 0x62), // Keypad 0
    (65, 0x63), // Keypad .
    (81, 0x67), // Keypad =
    // Modifiers.
    (59, 0xE0), // Left Control
    (56, 0xE1), // Left Shift
    (58, 0xE2), // Left Option
    (55, 0xE3), // Left Command
    (62, 0xE4), // Right Control
    (60, 0xE5), // Right Shift
    (61, 0xE6), // Right Option
    (54, 0xE7), // Right Command
];

/// The HID usage for a macOS virtual key, or `None` for keys we do not carry.
pub fn hid_from_vk(vk: u16) -> Option<KeyCode> {
    VK_HID
        .iter()
        .find(|&&(v, _)| v == vk)
        .map(|&(_, hid)| KeyCode::new(hid))
}

/// The macOS virtual key for a HID usage, or `None` if it has no key here.
pub fn vk_from_hid(code: KeyCode) -> Option<u16> {
    VK_HID
        .iter()
        .find(|&&(_, hid)| hid == code.value())
        .map(|&(vk, _)| vk)
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
    fn letters_use_their_hid_usages() {
        assert_eq!(hid_from_vk(0), Some(KeyCode::new(0x04))); // A
        assert_eq!(vk_from_hid(KeyCode::new(0x2C)), Some(49)); // Space
    }

    #[test]
    fn unmapped_keys_are_none() {
        assert_eq!(hid_from_vk(63), None); // fn key
        assert_eq!(vk_from_hid(KeyCode::new(0xFFFF)), None);
    }
}
