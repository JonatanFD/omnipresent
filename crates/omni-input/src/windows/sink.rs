//! Injection: synthesizing events with `SendInput`, as if they came from real
//! hardware. Used while this machine is the Target.
//!
//! Every injected event is stamped with our marker in `dwExtraInfo` and arrives
//! flagged "injected", so the capture hooks skip it and never echo our own
//! output back onto the wire.

use super::{PIXELS_PER_WHEEL_CLICK, WindowsInputError, keymap};
use crate::port::InputSink;
use omni_protocol::InputEvent;
use omni_protocol::input::{Action, MouseButton, ScrollDelta};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT,
    SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{SetCursorPos, WHEEL_DELTA, XBUTTON1, XBUTTON2};

/// Marker written to `dwExtraInfo` on everything we inject, so the capture
/// hooks can recognise and skip our own synthetic events.
pub(super) const INJECTED_MARKER: usize = 0x4F4D_4E49; // "OMNI"

/// Injects remote input into the local OS. The production `InputSink`.
#[derive(Debug, Default)]
pub struct WindowsSink {
    /// Sub-notch scroll remainders, so small pixel deltas accumulate into whole
    /// wheel notches instead of vanishing.
    scroll_rem_x: i32,
    scroll_rem_y: i32,
}

impl WindowsSink {
    /// Fallible for parity with the other platforms; building the sink on
    /// Windows cannot fail.
    pub fn new() -> Result<Self, WindowsInputError> {
        Ok(Self::default())
    }

    fn inject_key(
        &self,
        code: omni_protocol::KeyCode,
        action: Action,
    ) -> Result<(), WindowsInputError> {
        let Some(vk) = keymap::vk_from_hid(code) else {
            return Ok(()); // an unmapped key is dropped, never guessed
        };
        let mut flags = 0;
        if action == Action::Release {
            flags |= KEYEVENTF_KEYUP;
        }
        if keymap::is_extended_vk(vk) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: INJECTED_MARKER,
                },
            },
        };
        send(&input)
    }

    fn inject_mouse(
        &self,
        flags: u32,
        data: i32,
        dx: i32,
        dy: i32,
    ) -> Result<(), WindowsInputError> {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data as u32,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: INJECTED_MARKER,
                },
            },
        };
        send(&input)
    }

    fn inject_button(&self, button: MouseButton, action: Action) -> Result<(), WindowsInputError> {
        let (flags, data) = match (button, action) {
            (MouseButton::Left, Action::Press) => (MOUSEEVENTF_LEFTDOWN, 0),
            (MouseButton::Left, Action::Release) => (MOUSEEVENTF_LEFTUP, 0),
            (MouseButton::Right, Action::Press) => (MOUSEEVENTF_RIGHTDOWN, 0),
            (MouseButton::Right, Action::Release) => (MOUSEEVENTF_RIGHTUP, 0),
            (MouseButton::Middle, Action::Press) => (MOUSEEVENTF_MIDDLEDOWN, 0),
            (MouseButton::Middle, Action::Release) => (MOUSEEVENTF_MIDDLEUP, 0),
            (MouseButton::Back, Action::Press) => (MOUSEEVENTF_XDOWN, XBUTTON1 as i32),
            (MouseButton::Back, Action::Release) => (MOUSEEVENTF_XUP, XBUTTON1 as i32),
            (MouseButton::Forward, Action::Press) => (MOUSEEVENTF_XDOWN, XBUTTON2 as i32),
            (MouseButton::Forward, Action::Release) => (MOUSEEVENTF_XUP, XBUTTON2 as i32),
            (MouseButton::Other(_), _) => return Ok(()), // unknown button: dropped
        };
        self.inject_mouse(flags, data, 0, 0)
    }

    fn inject_scroll(&mut self, delta: ScrollDelta) -> Result<(), WindowsInputError> {
        self.scroll_rem_x += delta.dx;
        self.scroll_rem_y += delta.dy;
        let notches_x = self.scroll_rem_x / PIXELS_PER_WHEEL_CLICK;
        let notches_y = self.scroll_rem_y / PIXELS_PER_WHEEL_CLICK;
        self.scroll_rem_x -= notches_x * PIXELS_PER_WHEEL_CLICK;
        self.scroll_rem_y -= notches_y * PIXELS_PER_WHEEL_CLICK;
        if notches_y != 0 {
            self.inject_mouse(MOUSEEVENTF_WHEEL, notches_y * WHEEL_DELTA as i32, 0, 0)?;
        }
        if notches_x != 0 {
            self.inject_mouse(MOUSEEVENTF_HWHEEL, notches_x * WHEEL_DELTA as i32, 0, 0)?;
        }
        Ok(())
    }
}

impl InputSink for WindowsSink {
    type Error = WindowsInputError;

    fn inject(&mut self, event: InputEvent) -> Result<(), Self::Error> {
        match event {
            InputEvent::Key { code, action, .. } => self.inject_key(code, action),
            InputEvent::Motion(delta) => self.inject_mouse(MOUSEEVENTF_MOVE, 0, delta.dx, delta.dy),
            // Absolute placement from a remote controller. SetCursorPos drives a
            // drag as well as a move (a held button persists) and arrives
            // flagged "injected", so the capture hook skips it.
            InputEvent::Pointer { x, y } => self.warp(x, y),
            InputEvent::Button { button, action } => self.inject_button(button, action),
            InputEvent::Scroll(delta) => self.inject_scroll(delta),
        }
    }

    fn warp(&mut self, x: i32, y: i32) -> Result<(), Self::Error> {
        // Absolute placement on an edge crossing. SetCursorPos arrives flagged
        // "injected", so the capture hook skips it.
        if unsafe { SetCursorPos(x, y) } == 0 {
            return Err(WindowsInputError::Injection);
        }
        Ok(())
    }
}

/// Sends one synthesized event; fails if the OS accepted none (e.g. the input
/// was blocked by a more-privileged window — see the elevation note in
/// `diagnose`).
fn send(input: &INPUT) -> Result<(), WindowsInputError> {
    let sent = unsafe { SendInput(1, input, std::mem::size_of::<INPUT>() as i32) };
    if sent == 1 {
        Ok(())
    } else {
        Err(WindowsInputError::Injection)
    }
}
