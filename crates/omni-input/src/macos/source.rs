//! Capture: a CGEvent tap on a dedicated thread.
//!
//! The tap thread owns a CFRunLoop that delivers every keyboard and mouse
//! event to our callback, which translates it to a protocol event and queues
//! it for `poll`. While suppressed, the callback also *drops* the original
//! event so the local OS never acts on it — that is what keeps typing from
//! landing on both machines while a remote session is active.

use super::convert::{is_modifier_vk, modifiers_from_flags, toggle_modifier};
use super::{MacosInputError, keymap};
use crate::macos::convert::button_from_cg_number;
use crate::port::InputSource;
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{CFRunLoop, CFRunLoopRef, kCFRunLoopCommonModes};
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField,
};
use omni_protocol::InputEvent;
use omni_protocol::input::{Action, MouseDelta, ScrollDelta};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

// Re-enabling a tap the OS disabled (e.g. after a slow-callback timeout) needs
// the raw C call; the safe wrapper only exposes `enable` on the owning struct,
// which the callback cannot reach.
unsafe extern "C" {
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

/// The marker we stamp on events *we* inject (see the sink), so the tap can
/// ignore them instead of capturing our own synthetic input back.
pub(super) const INJECTED_MARKER: i64 = 0x4F4D4E49; // "OMNI"

/// Captures local keyboard and mouse input. The production `InputSource`.
#[derive(Debug)]
pub struct MacosSource {
    events: mpsc::Receiver<InputEvent>,
    suppressed: Arc<AtomicBool>,
    /// The tap thread's CFRunLoopRef, kept as a raw address so `Drop` (on
    /// another thread) can stop it. CFRunLoop is documented thread-safe.
    runloop: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
}

impl MacosSource {
    /// Starts the capture thread and installs the event tap. Fails if the tap
    /// cannot be created (no Accessibility permission).
    pub fn new() -> Result<Self, MacosInputError> {
        let (event_tx, events) = mpsc::channel::<InputEvent>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), MacosInputError>>();
        let suppressed = Arc::new(AtomicBool::new(false));
        let runloop = Arc::new(AtomicUsize::new(0));

        let thread_suppressed = suppressed.clone();
        let thread_runloop = runloop.clone();
        let thread = std::thread::Builder::new()
            .name("omni-input-tap".into())
            .spawn(move || run_tap(event_tx, ready_tx, thread_suppressed, thread_runloop))
            .map_err(|_| MacosInputError::TapCreation)?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                events,
                suppressed,
                runloop,
                thread: Some(thread),
            }),
            _ => {
                let _ = thread.join();
                Err(MacosInputError::TapCreation)
            }
        }
    }
}

impl InputSource for MacosSource {
    type Error = MacosInputError;

    fn poll(&mut self) -> Result<Option<InputEvent>, Self::Error> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(MacosInputError::CaptureStopped),
        }
    }

    fn set_suppressed(&mut self, suppressed: bool) {
        self.suppressed.store(suppressed, Ordering::Relaxed);
    }
}

impl Drop for MacosSource {
    fn drop(&mut self) {
        let ptr = self.runloop.swap(0, Ordering::AcqRel);
        if ptr != 0 {
            // Stop the tap thread's run loop so the thread can exit.
            let runloop = unsafe { CFRunLoop::wrap_under_get_rule(ptr as CFRunLoopRef) };
            runloop.stop();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The body of the tap thread: install the tap, publish readiness, run the
/// loop until stopped.
fn run_tap(
    events: mpsc::Sender<InputEvent>,
    ready: mpsc::Sender<Result<(), MacosInputError>>,
    suppressed: Arc<AtomicBool>,
    runloop_out: Arc<AtomicUsize>,
) {
    // The tap's own mach port, so the callback can re-enable it if the OS
    // disables it (timeout). Written once below, before any event arrives.
    let tap_port = Arc::new(AtomicUsize::new(0));
    let callback_port = tap_port.clone();
    // Held-modifier bookkeeping for FlagsChanged events (tap thread only).
    let held_modifiers = Cell::new(0u64);

    let interest = vec![
        CGEventType::KeyDown,
        CGEventType::KeyUp,
        CGEventType::FlagsChanged,
        CGEventType::MouseMoved,
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
        CGEventType::LeftMouseDragged,
        CGEventType::RightMouseDown,
        CGEventType::RightMouseUp,
        CGEventType::RightMouseDragged,
        CGEventType::OtherMouseDown,
        CGEventType::OtherMouseUp,
        CGEventType::OtherMouseDragged,
        CGEventType::ScrollWheel,
    ];

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        interest,
        move |_proxy, event_type, event| {
            match event_type {
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    let port = callback_port.load(Ordering::Acquire);
                    if port != 0 {
                        unsafe { CGEventTapEnable(port as CFMachPortRef, true) };
                    }
                    return CallbackResult::Keep;
                }
                _ => {}
            }
            // Never re-capture events this daemon injected itself.
            if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == INJECTED_MARKER
            {
                return CallbackResult::Keep;
            }
            if let Some(converted) = convert_event(event_type, event, &held_modifiers) {
                let _ = events.send(converted);
            }
            if suppressed.load(Ordering::Relaxed) {
                CallbackResult::Drop
            } else {
                CallbackResult::Keep
            }
        },
    );

    let tap = match tap {
        Ok(tap) => tap,
        Err(()) => {
            let _ = ready.send(Err(MacosInputError::TapCreation));
            return;
        }
    };

    let Ok(source) = tap.mach_port().create_runloop_source(0) else {
        let _ = ready.send(Err(MacosInputError::TapCreation));
        return;
    };
    tap_port.store(
        tap.mach_port().as_concrete_TypeRef() as usize,
        Ordering::Release,
    );

    let current = CFRunLoop::get_current();
    current.add_source(&source, unsafe { kCFRunLoopCommonModes });
    tap.enable();
    runloop_out.store(current.as_concrete_TypeRef() as usize, Ordering::Release);

    let _ = ready.send(Ok(()));
    CFRunLoop::run_current();
    // Run loop stopped (Drop): the tap is disabled when it goes out of scope.
}

/// Translates one CGEvent into the protocol vocabulary, or `None` for events
/// we do not carry (unmapped keys, zero scrolls...).
fn convert_event(
    event_type: CGEventType,
    event: &CGEvent,
    held_modifiers: &Cell<u64>,
) -> Option<InputEvent> {
    match event_type {
        CGEventType::KeyDown | CGEventType::KeyUp => {
            let vk = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            let code = keymap::hid_from_vk(vk)?;
            let action = if matches!(event_type, CGEventType::KeyDown) {
                Action::Press
            } else {
                Action::Release
            };
            Some(InputEvent::Key {
                code,
                action,
                modifiers: modifiers_from_flags(event.get_flags()),
            })
        }
        CGEventType::FlagsChanged => {
            let vk = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            if !is_modifier_vk(vk) {
                return None;
            }
            let code = keymap::hid_from_vk(vk)?;
            let (held, action) = toggle_modifier(held_modifiers.get(), vk);
            held_modifiers.set(held);
            Some(InputEvent::Key {
                code,
                action,
                modifiers: modifiers_from_flags(event.get_flags()),
            })
        }
        CGEventType::MouseMoved
        | CGEventType::LeftMouseDragged
        | CGEventType::RightMouseDragged
        | CGEventType::OtherMouseDragged => {
            let dx = event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_X) as i32;
            let dy = event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y) as i32;
            Some(InputEvent::Motion(MouseDelta::new(dx, dy)))
        }
        CGEventType::LeftMouseDown
        | CGEventType::LeftMouseUp
        | CGEventType::RightMouseDown
        | CGEventType::RightMouseUp
        | CGEventType::OtherMouseDown
        | CGEventType::OtherMouseUp => {
            let number = event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER);
            let action = if matches!(
                event_type,
                CGEventType::LeftMouseDown
                    | CGEventType::RightMouseDown
                    | CGEventType::OtherMouseDown
            ) {
                Action::Press
            } else {
                Action::Release
            };
            Some(InputEvent::Button {
                button: button_from_cg_number(number),
                action,
            })
        }
        CGEventType::ScrollWheel => {
            let dy = event
                .get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1)
                as i32;
            let dx = event
                .get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2)
                as i32;
            if dx == 0 && dy == 0 {
                None
            } else {
                Some(InputEvent::Scroll(ScrollDelta::new(dx, dy)))
            }
        }
        _ => None,
    }
}
