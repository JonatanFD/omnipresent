//! Mapping between Linux evdev key codes (`KEY_*`) and the USB HID usage codes
//! (keyboard page 0x07) that [`omni_protocol::KeyCode`] carries.

use omni_protocol::KeyCode;

/// Every (evdev key code, HID usage) pair we translate. One table, scanned in
/// both directions, so the two mappings can never disagree.
const EVDEV_HID: &[(u16, u32)] = &[
    (1, 0x29),   // KEY_ESC
    (2, 0x1E),   // KEY_1
    (3, 0x1F),   // KEY_2
    (4, 0x20),   // KEY_3
    (5, 0x21),   // KEY_4
    (6, 0x22),   // KEY_5
    (7, 0x23),   // KEY_6
    (8, 0x24),   // KEY_7
    (9, 0x25),   // KEY_8
    (10, 0x26),  // KEY_9
    (11, 0x27),  // KEY_0
    (12, 0x2D),  // KEY_MINUS
    (13, 0x2E),  // KEY_EQUAL
    (14, 0x2A),  // KEY_BACKSPACE
    (15, 0x2B),  // KEY_TAB
    (16, 0x14),  // KEY_Q
    (17, 0x1A),  // KEY_W
    (18, 0x08),  // KEY_E
    (19, 0x15),  // KEY_R
    (20, 0x17),  // KEY_T
    (21, 0x1C),  // KEY_Y
    (22, 0x18),  // KEY_U
    (23, 0x0C),  // KEY_I
    (24, 0x12),  // KEY_O
    (25, 0x13),  // KEY_P
    (26, 0x2F),  // KEY_LEFTBRACE
    (27, 0x30),  // KEY_RIGHTBRACE
    (28, 0x28),  // KEY_ENTER
    (29, 0xE0),  // KEY_LEFTCTRL
    (30, 0x04),  // KEY_A
    (31, 0x16),  // KEY_S
    (32, 0x07),  // KEY_D
    (33, 0x09),  // KEY_F
    (34, 0x0A),  // KEY_G
    (35, 0x0B),  // KEY_H
    (36, 0x0D),  // KEY_J
    (37, 0x0E),  // KEY_K
    (38, 0x0F),  // KEY_L
    (39, 0x33),  // KEY_SEMICOLON
    (40, 0x34),  // KEY_APOSTROPHE
    (41, 0x35),  // KEY_GRAVE
    (42, 0xE1),  // KEY_LEFTSHIFT
    (43, 0x31),  // KEY_BACKSLASH
    (44, 0x1D),  // KEY_Z
    (45, 0x1B),  // KEY_X
    (46, 0x06),  // KEY_C
    (47, 0x19),  // KEY_V
    (48, 0x05),  // KEY_B
    (49, 0x11),  // KEY_N
    (50, 0x10),  // KEY_M
    (51, 0x36),  // KEY_COMMA
    (52, 0x37),  // KEY_DOT
    (53, 0x38),  // KEY_SLASH
    (54, 0xE5),  // KEY_RIGHTSHIFT
    (55, 0x55),  // KEY_KPASTERISK
    (56, 0xE2),  // KEY_LEFTALT
    (57, 0x2C),  // KEY_SPACE
    (58, 0x39),  // KEY_CAPSLOCK
    (59, 0x3A),  // KEY_F1
    (60, 0x3B),  // KEY_F2
    (61, 0x3C),  // KEY_F3
    (62, 0x3D),  // KEY_F4
    (63, 0x3E),  // KEY_F5
    (64, 0x3F),  // KEY_F6
    (65, 0x40),  // KEY_F7
    (66, 0x41),  // KEY_F8
    (67, 0x42),  // KEY_F9
    (68, 0x43),  // KEY_F10
    (69, 0x53),  // KEY_NUMLOCK
    (70, 0x47),  // KEY_SCROLLLOCK
    (71, 0x5F),  // KEY_KP7
    (72, 0x60),  // KEY_KP8
    (73, 0x61),  // KEY_KP9
    (74, 0x56),  // KEY_KPMINUS
    (75, 0x5C),  // KEY_KP4
    (76, 0x5D),  // KEY_KP5
    (77, 0x5E),  // KEY_KP6
    (78, 0x57),  // KEY_KPPLUS
    (79, 0x59),  // KEY_KP1
    (80, 0x5A),  // KEY_KP2
    (81, 0x5B),  // KEY_KP3
    (82, 0x62),  // KEY_KP0
    (83, 0x63),  // KEY_KPDOT
    (86, 0x64),  // KEY_102ND (non-US backslash)
    (87, 0x44),  // KEY_F11
    (88, 0x45),  // KEY_F12
    (96, 0x58),  // KEY_KPENTER
    (97, 0xE4),  // KEY_RIGHTCTRL
    (98, 0x54),  // KEY_KPSLASH
    (99, 0x46),  // KEY_SYSRQ (print screen)
    (100, 0xE6), // KEY_RIGHTALT
    (102, 0x4A), // KEY_HOME
    (103, 0x52), // KEY_UP
    (104, 0x4B), // KEY_PAGEUP
    (105, 0x50), // KEY_LEFT
    (106, 0x4F), // KEY_RIGHT
    (107, 0x4D), // KEY_END
    (108, 0x51), // KEY_DOWN
    (109, 0x4E), // KEY_PAGEDOWN
    (110, 0x49), // KEY_INSERT
    (111, 0x4C), // KEY_DELETE
    (117, 0x67), // KEY_KPEQUAL
    (119, 0x48), // KEY_PAUSE
    (125, 0xE3), // KEY_LEFTMETA
    (126, 0xE7), // KEY_RIGHTMETA
    (127, 0x65), // KEY_COMPOSE (menu)
    (183, 0x68), // KEY_F13
    (184, 0x69), // KEY_F14
    (185, 0x6A), // KEY_F15
    (186, 0x6B), // KEY_F16
    (187, 0x6C), // KEY_F17
    (188, 0x6D), // KEY_F18
    (189, 0x6E), // KEY_F19
    (190, 0x6F), // KEY_F20
];

/// The HID usage for an evdev key code, or `None` for keys we do not carry.
pub fn hid_from_evdev(code: u16) -> Option<KeyCode> {
    EVDEV_HID
        .iter()
        .find(|&&(c, _)| c == code)
        .map(|&(_, hid)| KeyCode::new(hid))
}

/// The evdev key code for a HID usage, or `None` if it has no key here.
pub fn evdev_from_hid(code: KeyCode) -> Option<u16> {
    EVDEV_HID
        .iter()
        .find(|&&(_, hid)| hid == code.value())
        .map(|&(c, _)| c)
}

/// Every evdev key code we can inject — the set a virtual keyboard registers.
pub fn injectable_keys() -> impl Iterator<Item = u16> {
    EVDEV_HID.iter().map(|&(code, _)| code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn table_has_no_duplicate_entries() {
        let codes: HashSet<u16> = EVDEV_HID.iter().map(|&(c, _)| c).collect();
        let hids: HashSet<u32> = EVDEV_HID.iter().map(|&(_, hid)| hid).collect();
        assert_eq!(codes.len(), EVDEV_HID.len(), "duplicate evdev code");
        assert_eq!(hids.len(), EVDEV_HID.len(), "duplicate HID usage");
    }

    #[test]
    fn every_mapping_round_trips() {
        for &(code, hid) in EVDEV_HID {
            let key = hid_from_evdev(code).expect("forward mapping");
            assert_eq!(key.value(), hid);
            assert_eq!(evdev_from_hid(key), Some(code));
        }
    }

    #[test]
    fn unmapped_keys_are_none() {
        assert_eq!(hid_from_evdev(240), None); // KEY_UNKNOWN
        assert_eq!(evdev_from_hid(KeyCode::new(0xFFFF)), None);
    }
}
