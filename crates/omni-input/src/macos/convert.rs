//! Pure translation between CGEvent vocabulary and the protocol's: modifier
//! flags, mouse buttons, and the press/release bookkeeping for modifier keys.
//! No OS calls here, so all of it is unit-testable.

use core_graphics::event::CGEventFlags;
use omni_protocol::input::{Action, Modifiers, MouseButton};
use std::time::Duration;

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

/// The running click count for a new mouse-down, written to a CGEvent's
/// `kCGMouseEventClickState`. macOS reads this field to recognize gestures:
/// 1 = single, 2 = double, 3 = triple, and so on. Real hardware gets it from
/// the OS, but a synthesized click must compute it — otherwise every click is
/// a fresh single click and double-click never fires.
///
/// A press continues the streak (count + 1) only when it is the *same* button
/// as the previous click, lands within the double-click `interval`, and barely
/// moved (`distance` within `max_distance`). Anything else restarts at 1.
pub fn next_click_count(
    same_button: bool,
    elapsed: Duration,
    distance: f64,
    previous_count: i64,
    interval: Duration,
    max_distance: f64,
) -> i64 {
    if same_button && elapsed <= interval && distance <= max_distance {
        previous_count + 1
    } else {
        1
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

    const INTERVAL: Duration = Duration::from_millis(500);
    const DISTANCE: f64 = 4.0;

    #[test]
    fn a_quick_same_spot_click_continues_the_streak() {
        // single -> double -> triple as long as each press is in time and place.
        let double = next_click_count(true, Duration::from_millis(120), 1.0, 1, INTERVAL, DISTANCE);
        assert_eq!(double, 2);
        let triple = next_click_count(true, Duration::from_millis(120), 1.0, 2, INTERVAL, DISTANCE);
        assert_eq!(triple, 3);
    }

    #[test]
    fn a_slow_click_restarts_at_a_single() {
        let count = next_click_count(true, Duration::from_millis(900), 1.0, 1, INTERVAL, DISTANCE);
        assert_eq!(count, 1);
    }

    #[test]
    fn a_click_that_moved_too_far_restarts_at_a_single() {
        let count = next_click_count(
            true,
            Duration::from_millis(120),
            50.0,
            1,
            INTERVAL,
            DISTANCE,
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn a_different_button_restarts_at_a_single() {
        let count = next_click_count(
            false,
            Duration::from_millis(120),
            0.0,
            1,
            INTERVAL,
            DISTANCE,
        );
        assert_eq!(count, 1);
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
