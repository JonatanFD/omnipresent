//! Injection: synthesizing events with `CGEventPost`, as if they came from
//! real hardware. Used while this machine is the Target.

use super::convert::{cg_button_number, flags_from_modifiers, next_click_count};
use super::source::INJECTED_MARKER;
use super::{MacosInputError, keymap};
use crate::port::InputSink;
use core_graphics::display::CGDisplay;
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use omni_protocol::InputEvent;
use omni_protocol::input::{Action, Modifiers, MouseButton, MouseDelta, ScrollDelta};
use std::time::{Duration, Instant};

// Posting a `MouseMoved` event repositions the cursor for applications, but
// macOS will not keep the hardware cursor *drawn* for moves that come purely
// from synthesized events with no physical device behind them — it blanks it.
// Warping the cursor to the same spot forces it to be drawn there, and
// re-associating cancels the brief post-warp suppression so a controlled
// machine stays responsive. This is what keeps the remote cursor visible.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
    fn CGAssociateMouseAndMouseCursorPosition(connected: i32) -> i32;
}

/// Forces the visible cursor to `position` so a synthesized move is actually
/// drawn there, then re-associates so the cursor keeps tracking.
fn keep_cursor_visible(position: DisplayPoint) {
    unsafe {
        CGWarpMouseCursorPosition(position.into());
        CGAssociateMouseAndMouseCursorPosition(1);
    }
}

/// macOS's default double-click interval: a second click within this window
/// (and barely moved) is the second click of a double-click.
const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(500);
/// How far the pointer may move between two clicks and still count as a
/// double-click, in display pixels.
const DOUBLE_CLICK_DISTANCE: f64 = 4.0;

/// The previous click, used to recognize double- and triple-clicks.
#[derive(Debug, Clone, Copy)]
struct LastClick {
    button: i64,
    at: Instant,
    position: DisplayPoint,
    count: i64,
}

/// Injects remote input into the local OS. The production `InputSink`.
///
/// Holds only plain state (which buttons are down, the last key modifiers, the
/// last click for double-click tracking), so it is freely sendable between
/// threads; every injection creates its own event source.
#[derive(Debug, Default)]
pub struct MacosSink {
    /// CG button numbers currently held, as a bitmask. Decides whether motion
    /// is a move or a drag.
    held_buttons: u32,
    /// Modifiers from the most recent key event, applied to mouse events so
    /// modifier-clicks (e.g. Cmd-click) work.
    modifiers: Modifiers,
    /// The last button press, so a quick second press at the same spot is
    /// tagged as a double-click rather than two single clicks.
    last_click: Option<LastClick>,
}

impl MacosSink {
    /// Fallible for parity with the Linux sink (which must open uinput);
    /// creating event sources on macOS cannot fail.
    pub fn new() -> Result<Self, MacosInputError> {
        Ok(Self::default())
    }

    fn inject_key(
        &mut self,
        code: omni_protocol::KeyCode,
        action: Action,
        modifiers: Modifiers,
    ) -> Result<(), MacosInputError> {
        let Some(vk) = keymap::vk_from_hid(code) else {
            // An unmapped key is dropped, never guessed.
            return Ok(());
        };
        self.modifiers = modifiers;
        let event = CGEvent::new_keyboard_event(event_source()?, vk, action == Action::Press)
            .map_err(|_| MacosInputError::EventCreation)?;
        event.set_flags(flags_from_modifiers(modifiers));
        post(&event);
        Ok(())
    }

    fn inject_motion(&mut self, delta: MouseDelta) -> Result<(), MacosInputError> {
        let position = clamp_to_display(cursor_position()?.offset(delta));
        self.move_to(position)
    }

    /// Places the cursor at an absolute position on this screen. This is what a
    /// remote controller drives — no read-back of the local cursor, so there is
    /// nothing to drift.
    fn inject_pointer(&mut self, x: i32, y: i32) -> Result<(), MacosInputError> {
        let position = clamp_to_display(DisplayPoint {
            x: x as f64,
            y: y as f64,
        });
        self.move_to(position)
    }

    /// Posts a move (or a drag, if a button is held) to `position`.
    fn move_to(&mut self, position: DisplayPoint) -> Result<(), MacosInputError> {
        let (event_type, button) = if self.held_buttons & 1 != 0 {
            (CGEventType::LeftMouseDragged, CGMouseButton::Left)
        } else if self.held_buttons & 2 != 0 {
            (CGEventType::RightMouseDragged, CGMouseButton::Right)
        } else if self.held_buttons != 0 {
            (CGEventType::OtherMouseDragged, CGMouseButton::Center)
        } else {
            (CGEventType::MouseMoved, CGMouseButton::Left)
        };
        let event = CGEvent::new_mouse_event(event_source()?, event_type, position.into(), button)
            .map_err(|_| MacosInputError::EventCreation)?;
        event.set_flags(flags_from_modifiers(self.modifiers));
        post(&event);
        keep_cursor_visible(position);
        Ok(())
    }

    fn inject_button(
        &mut self,
        button: MouseButton,
        action: Action,
    ) -> Result<(), MacosInputError> {
        let number = cg_button_number(button);
        let (event_type, cg_button) = match (number, action) {
            (0, Action::Press) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
            (0, Action::Release) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
            (1, Action::Press) => (CGEventType::RightMouseDown, CGMouseButton::Right),
            (1, Action::Release) => (CGEventType::RightMouseUp, CGMouseButton::Right),
            (_, Action::Press) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
            (_, Action::Release) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
        };
        let position = clamp_to_display(cursor_position()?);
        let click_state = self.click_state(number, action, position);
        let event =
            CGEvent::new_mouse_event(event_source()?, event_type, position.into(), cg_button)
                .map_err(|_| MacosInputError::EventCreation)?;
        if number >= 2 {
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number);
        }
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_state);
        event.set_flags(flags_from_modifiers(self.modifiers));
        post(&event);

        let bit = 1u32 << number.clamp(0, 31);
        match action {
            Action::Press => self.held_buttons |= bit,
            Action::Release => self.held_buttons &= !bit,
        }
        Ok(())
    }

    /// The click count to stamp on this button event. A press computes the
    /// running streak (single/double/triple) from the previous click and
    /// records itself; the matching release carries the same count, so macOS
    /// sees a consistent `down(2)/up(2)` for the second click of a double.
    fn click_state(&mut self, button: i64, action: Action, position: DisplayPoint) -> i64 {
        match action {
            Action::Press => {
                let now = Instant::now();
                let count = match self.last_click {
                    Some(last) => next_click_count(
                        last.button == button,
                        now.saturating_duration_since(last.at),
                        position.distance_to(last.position),
                        last.count,
                        DOUBLE_CLICK_INTERVAL,
                        DOUBLE_CLICK_DISTANCE,
                    ),
                    None => 1,
                };
                self.last_click = Some(LastClick {
                    button,
                    at: now,
                    position,
                    count,
                });
                count
            }
            Action::Release => match self.last_click {
                Some(last) if last.button == button => last.count,
                _ => 1,
            },
        }
    }

    fn inject_scroll(&mut self, delta: ScrollDelta) -> Result<(), MacosInputError> {
        // Wheel 1 is the vertical axis, wheel 2 the horizontal, in pixels —
        // mirroring what the source captures.
        let event = CGEvent::new_scroll_event(
            event_source()?,
            ScrollEventUnit::PIXEL,
            2,
            delta.dy,
            delta.dx,
            0,
        )
        .map_err(|_| MacosInputError::EventCreation)?;
        post(&event);
        Ok(())
    }
}

impl InputSink for MacosSink {
    type Error = MacosInputError;

    fn inject(&mut self, event: InputEvent) -> Result<(), Self::Error> {
        match event {
            InputEvent::Key {
                code,
                action,
                modifiers,
            } => self.inject_key(code, action, modifiers),
            InputEvent::Motion(delta) => self.inject_motion(delta),
            InputEvent::Pointer { x, y } => self.inject_pointer(x, y),
            InputEvent::Button { button, action } => self.inject_button(button, action),
            InputEvent::Scroll(delta) => self.inject_scroll(delta),
        }
    }

    fn warp(&mut self, x: i32, y: i32) -> Result<(), Self::Error> {
        let position = clamp_to_display(DisplayPoint {
            x: x as f64,
            y: y as f64,
        });
        let event = CGEvent::new_mouse_event(
            event_source()?,
            CGEventType::MouseMoved,
            position.into(),
            CGMouseButton::Left,
        )
        .map_err(|_| MacosInputError::EventCreation)?;
        post(&event);
        keep_cursor_visible(position);
        Ok(())
    }
}

/// A plain point in global display coordinates.
#[derive(Debug, Clone, Copy)]
struct DisplayPoint {
    x: f64,
    y: f64,
}

impl DisplayPoint {
    fn offset(self, delta: MouseDelta) -> DisplayPoint {
        DisplayPoint {
            x: self.x + delta.dx as f64,
            y: self.y + delta.dy as f64,
        }
    }

    /// Straight-line distance to another point, for the double-click check.
    fn distance_to(self, other: DisplayPoint) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

impl From<DisplayPoint> for CGPoint {
    fn from(point: DisplayPoint) -> CGPoint {
        CGPoint::new(point.x, point.y)
    }
}

/// Stamps and posts an event at the HID level.
fn post(event: &CGEvent) {
    // Mark the event as ours so the capture tap never re-captures it.
    event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, INJECTED_MARKER);
    event.post(CGEventTapLocation::HID);
}

fn event_source() -> Result<CGEventSource, MacosInputError> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| MacosInputError::EventCreation)
}

fn cursor_position() -> Result<DisplayPoint, MacosInputError> {
    let location = CGEvent::new(event_source()?)
        .map_err(|_| MacosInputError::EventCreation)?
        .location();
    Ok(DisplayPoint {
        x: location.x,
        y: location.y,
    })
}

/// Keeps an injected position on the main display.
fn clamp_to_display(point: DisplayPoint) -> DisplayPoint {
    let bounds = CGDisplay::main().bounds();
    DisplayPoint {
        x: point
            .x
            .clamp(bounds.origin.x, bounds.origin.x + bounds.size.width - 1.0),
        y: point
            .y
            .clamp(bounds.origin.y, bounds.origin.y + bounds.size.height - 1.0),
    }
}
