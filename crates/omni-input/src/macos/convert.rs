//! Pure translation between CGEvent vocabulary and the protocol's: modifier
//! flags, mouse buttons, and the press/release bookkeeping for modifier keys.
//! No OS calls here, so all of it is unit-testable.

use core_graphics::event::CGEventFlags;
use omni_protocol::input::{Action, Modifiers, MouseButton};

/// Protocol modifiers for a CGEvent flag set.
pub fn modifiers_from_flags(flags: CGEventFlags) -> Modifiers {
    let mut modifiers = Modifiers::NONE;
    if flags.contains(CGEventFlags::CGEventFlagShift) {
        modifiers = modifiers.with(Modifiers::SHIFT);
    }
    if flags.contains(CGEventFlags::CGEventFlagControl) {
        modifiers = modifiers.with(Modifiers::CONTROL);
    }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) {
        modifiers = modifiers.with(Modifiers::ALT);
    }
    if flags.contains(CGEventFlags::CGEventFlagCommand) {
        modifiers = modifiers.with(Modifiers::META);
    }
    modifiers
}

/// CGEvent flags for a set of protocol modifiers.
pub fn flags_from_modifiers(modifiers: Modifiers) -> CGEventFlags {
    let mut flags = CGEventFlags::CGEventFlagNull;
    if modifiers.contains(Modifiers::SHIFT) {
        flags |= CGEventFlags::CGEventFlagShift;
    }
    if modifiers.contains(Modifiers::CONTROL) {
        flags |= CGEventFlags::CGEventFlagControl;
    }
    if modifiers.contains(Modifiers::ALT) {
        flags |= CGEventFlags::CGEventFlagAlternate;
    }
    if modifiers.contains(Modifiers::META) {
        flags |= CGEventFlags::CGEventFlagCommand;
    }
    flags
}

/// Whether a macOS virtual key is a modifier key (reported via `FlagsChanged`
/// rather than `KeyDown`/`KeyUp`).
pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 54..=62)
}

/// Press/release bookkeeping for modifier keys. `FlagsChanged` events do not
/// say whether the key went down or up, so we track which modifier keys are
/// held in a bitmask (indexed by virtual key, all of which are < 64) and
/// toggle: an unseen key is a press, a held key is a release.
pub fn toggle_modifier(held: u64, vk: u16) -> (u64, Action) {
    let bit = 1u64 << (vk % 64);
    if held & bit != 0 {
        (held & !bit, Action::Release)
    } else {
        (held | bit, Action::Press)
    }
}

/// The CG button number for a protocol mouse button (CG numbers buttons
/// 0 = left, 1 = right, 2 = middle, then 3, 4, ... for extras).
pub fn cg_button_number(button: MouseButton) -> i64 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(n) => n as i64,
    }
}

/// The protocol mouse button for a CG button number.
pub fn button_from_cg_number(number: i64) -> MouseButton {
    match number {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        n => MouseButton::Other(n.clamp(0, u8::MAX as i64) as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_and_modifiers_round_trip() {
        let combos = [
            Modifiers::NONE,
            Modifiers::SHIFT,
            Modifiers::CONTROL.with(Modifiers::ALT),
            Modifiers::SHIFT
                .with(Modifiers::CONTROL)
                .with(Modifiers::ALT)
                .with(Modifiers::META),
        ];
        for modifiers in combos {
            assert_eq!(
                modifiers_from_flags(flags_from_modifiers(modifiers)),
                modifiers
            );
        }
    }

    #[test]
    fn unrelated_flags_are_ignored() {
        let flags = CGEventFlags::CGEventFlagShift | CGEventFlags::CGEventFlagNumericPad;
        assert_eq!(modifiers_from_flags(flags), Modifiers::SHIFT);
    }

    #[test]
    fn modifier_keys_toggle_between_press_and_release() {
        let (held, action) = toggle_modifier(0, 56); // left shift down
        assert_eq!(action, Action::Press);
        let (held, action) = toggle_modifier(held, 60); // right shift down
        assert_eq!(action, Action::Press);
        let (held, action) = toggle_modifier(held, 56); // left shift up
        assert_eq!(action, Action::Release);
        let (held, action) = toggle_modifier(held, 60); // right shift up
        assert_eq!(action, Action::Release);
        assert_eq!(held, 0);
    }

    #[test]
    fn buttons_round_trip_through_cg_numbers() {
        for button in [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
            MouseButton::Other(7),
        ] {
            assert_eq!(button_from_cg_number(cg_button_number(button)), button);
        }
    }
}
