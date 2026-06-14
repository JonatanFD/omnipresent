//! Linux adapters for the input ports, over evdev and uinput.
//!
//! - [`LinuxSource`] reads keyboards and mice directly from `/dev/input`
//!   (evdev), one reader thread per device. While suppressed it *grabs* the
//!   devices (`EVIOCGRAB`) so events reach only us, not the local desktop.
//! - [`LinuxSink`] injects events through a uinput virtual device.
//!
//! Least privilege: both ends need only read access to `/dev/input/event*`
//! and write access to `/dev/uinput` — typically membership in the `input`
//! group, never root.

pub mod keymap;

use crate::port::{InputSink, InputSource};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, EventSummary, EventType, RelativeAxisCode};
use omni_protocol::InputEvent;
use omni_protocol::input::{Action, Modifiers, MouseButton, MouseDelta, ScrollDelta};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc;

/// The name our uinput virtual device registers under. The capture side skips
/// any device with this name so we never re-capture our own injections.
const VIRTUAL_DEVICE_NAME: &str = "omnipresent-virtual-input";

/// How many scroll "pixels" one wheel click carries, aligning evdev's
/// line-based wheel with the pixel-based deltas other platforms report.
const PIXELS_PER_WHEEL_CLICK: i32 = 24;

/// Why a Linux input operation failed.
#[derive(Debug)]
pub enum LinuxInputError {
    /// No readable keyboard or mouse was found in /dev/input. Usually a
    /// permission problem: the daemon's user must be in the `input` group.
    NoDevices,
    /// Every capture thread is gone, so no more events will ever arrive.
    CaptureStopped,
    /// Creating or writing the uinput virtual device failed. Usually missing
    /// write access to /dev/uinput.
    Inject(std::io::Error),
}

impl std::fmt::Display for LinuxInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinuxInputError::NoDevices => f.write_str(
                "no readable input devices in /dev/input — add the user to the `input` group",
            ),
            LinuxInputError::CaptureStopped => f.write_str("input capture stopped"),
            LinuxInputError::Inject(e) => {
                write!(f, "could not inject through uinput (/dev/uinput): {e}")
            }
        }
    }
}

impl std::error::Error for LinuxInputError {}

static XLIB: std::sync::OnceLock<Result<x11_dl::xlib::Xlib, x11_dl::error::OpenError>> =
    std::sync::OnceLock::new();

unsafe extern "C" fn io_error_handler(_display: *mut x11_dl::xlib::Display) -> std::os::raw::c_int {
    tracing::error!("X11 fatal I/O error: connection to X server lost.");
    0
}

fn get_xlib() -> Option<&'static x11_dl::xlib::Xlib> {
    XLIB.get_or_init(|| {
        let xlib = x11_dl::xlib::Xlib::open()?;
        unsafe {
            (xlib.XInitThreads)();
            (xlib.XSetIOErrorHandler)(Some(io_error_handler));
        }
        Ok(xlib)
    })
    .as_ref()
    .ok()
}

struct X11CursorState {
    xlib: &'static x11_dl::xlib::Xlib,
    display: *mut x11_dl::xlib::Display,
    root_window: x11_dl::xlib::Window,
    cursor: x11_dl::xlib::Cursor,
    pixmap: x11_dl::xlib::Pixmap,
}

// Safety: X11CursorState is only ever accessed from the single capture thread.
// The Display pointer is opened and closed on that same thread; Xlib itself
// requires XInitThreads() for concurrent use, but we never share this across
// threads concurrently.
unsafe impl Send for X11CursorState {}

impl std::fmt::Debug for X11CursorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X11CursorState").finish_non_exhaustive()
    }
}

impl X11CursorState {
    fn hide() -> Option<Self> {
        let xlib = get_xlib()?;
        unsafe {
            let display = (xlib.XOpenDisplay)(std::ptr::null());
            if display.is_null() {
                tracing::warn!(
                    "Failed to open X11 display. If running under Wayland, global cursor \
                     hiding is restricted by the compositor."
                );
                return None;
            }
            let root_window = (xlib.XDefaultRootWindow)(display);
            let data = [0u8; 1];
            let pixmap = (xlib.XCreateBitmapFromData)(
                display,
                root_window,
                data.as_ptr() as *const std::os::raw::c_char,
                1,
                1,
            );
            if pixmap == 0 {
                (xlib.XCloseDisplay)(display);
                return None;
            }
            let mut color: x11_dl::xlib::XColor = std::mem::zeroed();
            let cursor =
                (xlib.XCreatePixmapCursor)(display, pixmap, pixmap, &mut color, &mut color, 0, 0);
            if cursor == 0 {
                (xlib.XFreePixmap)(display, pixmap);
                (xlib.XCloseDisplay)(display);
                return None;
            }
            (xlib.XDefineCursor)(display, root_window, cursor);
            (xlib.XFlush)(display);

            Some(Self {
                xlib,
                display,
                root_window,
                cursor,
                pixmap,
            })
        }
    }
}

impl Drop for X11CursorState {
    fn drop(&mut self) {
        unsafe {
            (self.xlib.XUndefineCursor)(self.display, self.root_window);
            (self.xlib.XFreeCursor)(self.display, self.cursor);
            (self.xlib.XFreePixmap)(self.display, self.pixmap);
            (self.xlib.XCloseDisplay)(self.display);
        }
    }
}

/// Captures local keyboard and mouse input. The production `InputSource`.
#[derive(Debug)]
pub struct LinuxSource {
    events: mpsc::Receiver<InputEvent>,
    suppressed: Arc<AtomicBool>,
    // RAII guard for the hidden X11 cursor. Dropping this restores the normal cursor.
    x11_state: Option<X11CursorState>,
}

impl LinuxSource {
    /// Opens every keyboard and mouse in /dev/input and starts one reader
    /// thread per device.
    pub fn new() -> Result<Self, LinuxInputError> {
        let (tx, events) = mpsc::channel();
        let suppressed = Arc::new(AtomicBool::new(false));
        let modifiers = Arc::new(AtomicU8::new(0));

        let mut readers = 0;
        for (_path, device) in evdev::enumerate() {
            if !is_keyboard(&device) && !is_mouse(&device) {
                continue;
            }
            if device.name() == Some(VIRTUAL_DEVICE_NAME) {
                continue;
            }
            let tx = tx.clone();
            let suppressed = suppressed.clone();
            let modifiers = modifiers.clone();
            let spawned = std::thread::Builder::new()
                .name("omni-input-evdev".into())
                .spawn(move || read_device(device, tx, suppressed, modifiers));
            if spawned.is_ok() {
                readers += 1;
            }
        }
        if readers == 0 {
            return Err(LinuxInputError::NoDevices);
        }
        Ok(Self {
            events,
            suppressed,
            x11_state: None,
        })
    }
}

impl InputSource for LinuxSource {
    type Error = LinuxInputError;

    fn poll(&mut self) -> Result<Option<InputEvent>, Self::Error> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(LinuxInputError::CaptureStopped),
        }
    }

    fn set_suppressed(&mut self, suppressed: bool) {
        let was_suppressed = self.suppressed.swap(suppressed, Ordering::Relaxed);
        if suppressed {
            if !was_suppressed && self.x11_state.is_none() {
                self.x11_state = X11CursorState::hide();
            }
        } else {
            self.x11_state = None; // Dropping cleans up X11 cursor
        }
    }
}

/// A device that can type: it reports KEY_A.
fn is_keyboard(device: &Device) -> bool {
    device
        .supported_keys()
        .is_some_and(|keys| keys.contains(evdev::KeyCode::KEY_A))
}

/// A device that can point: relative X/Y plus a left button.
fn is_mouse(device: &Device) -> bool {
    let has_motion = device
        .supported_relative_axes()
        .is_some_and(|axes| axes.contains(RelativeAxisCode::REL_X));
    let has_button = device
        .supported_keys()
        .is_some_and(|keys| keys.contains(evdev::KeyCode::BTN_LEFT));
    has_motion && has_button
}

/// One device's read loop: convert and forward every event, and keep the grab
/// state in line with suppression. Exits when the source is dropped (the
/// channel closes) or the device goes away.
fn read_device(
    mut device: Device,
    events: mpsc::Sender<InputEvent>,
    suppressed: Arc<AtomicBool>,
    modifiers: Arc<AtomicU8>,
) {
    let mut grabbed = false;
    // Relative motion accumulated within one hardware report frame.
    let mut pending = MouseDelta::new(0, 0);

    loop {
        let want_grab = suppressed.load(Ordering::Relaxed);
        if want_grab != grabbed {
            let done = if want_grab {
                device.grab()
            } else {
                device.ungrab()
            };
            if done.is_ok() {
                grabbed = want_grab;
            }
        }

        let batch = match device.fetch_events() {
            Ok(iter) => iter.collect::<Vec<_>>(),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return, // device unplugged or fd revoked
        };

        for raw in batch {
            if let Some(event) = convert_event(raw, &mut pending, &modifiers)
                && events.send(event).is_err()
            {
                return; // the source was dropped
            }
        }
    }
}

/// Translates one evdev event. Motion axes are accumulated into `pending` and
/// flushed as a single `Motion` on the frame's SYN_REPORT.
fn convert_event(
    raw: evdev::InputEvent,
    pending: &mut MouseDelta,
    modifiers: &Arc<AtomicU8>,
) -> Option<InputEvent> {
    match raw.destructure() {
        EventSummary::Key(_, key, value) => {
            // value: 1 = press, 0 = release, 2 = autorepeat.
            let action = if value == 0 {
                Action::Release
            } else {
                Action::Press
            };
            if let Some(button) = button_from_evdev(key.0) {
                return Some(InputEvent::Button { button, action });
            }
            if value == 2 && modifier_mask(key.0).is_some() {
                return None; // modifier autorepeat carries no information
            }
            update_modifiers(modifiers, key.0, action);
            let code = keymap::hid_from_evdev(key.0)?;
            Some(InputEvent::Key {
                code,
                action,
                modifiers: held_modifiers(modifiers),
            })
        }
        EventSummary::RelativeAxis(_, axis, value) => match axis {
            RelativeAxisCode::REL_X => {
                pending.dx += value;
                None
            }
            RelativeAxisCode::REL_Y => {
                pending.dy += value;
                None
            }
            RelativeAxisCode::REL_WHEEL => Some(InputEvent::Scroll(ScrollDelta::new(
                0,
                value * PIXELS_PER_WHEEL_CLICK,
            ))),
            RelativeAxisCode::REL_HWHEEL => Some(InputEvent::Scroll(ScrollDelta::new(
                value * PIXELS_PER_WHEEL_CLICK,
                0,
            ))),
            _ => None,
        },
        EventSummary::Synchronization(..) => {
            if pending.dx != 0 || pending.dy != 0 {
                let motion = InputEvent::Motion(*pending);
                *pending = MouseDelta::new(0, 0);
                Some(motion)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn button_from_evdev(code: u16) -> Option<MouseButton> {
    match code {
        0x110 => Some(MouseButton::Left),
        0x111 => Some(MouseButton::Right),
        0x112 => Some(MouseButton::Middle),
        0x113 => Some(MouseButton::Back),
        0x114 => Some(MouseButton::Forward),
        _ => None,
    }
}

fn evdev_from_button(button: MouseButton) -> Option<u16> {
    match button {
        MouseButton::Left => Some(0x110),
        MouseButton::Right => Some(0x111),
        MouseButton::Middle => Some(0x112),
        MouseButton::Back => Some(0x113),
        MouseButton::Forward => Some(0x114),
        MouseButton::Other(_) => None,
    }
}

/// The held-modifier bit a key code controls, if it is a modifier key. The
/// bit positions mirror the `Modifiers` constants (shift, ctrl, alt, meta).
fn modifier_mask(code: u16) -> Option<u8> {
    match code {
        42 | 54 => Some(1 << 0),   // shift
        29 | 97 => Some(1 << 1),   // ctrl
        56 | 100 => Some(1 << 2),  // alt
        125 | 126 => Some(1 << 3), // meta / super
        _ => None,
    }
}

/// Keeps the shared held-modifier byte in sync with modifier key presses.
fn update_modifiers(modifiers: &Arc<AtomicU8>, code: u16, action: Action) {
    let Some(mask) = modifier_mask(code) else {
        return;
    };
    match action {
        Action::Press => modifiers.fetch_or(mask, Ordering::Relaxed),
        Action::Release => modifiers.fetch_and(!mask, Ordering::Relaxed),
    };
}

/// The currently held modifiers, as the protocol type.
fn held_modifiers(modifiers: &Arc<AtomicU8>) -> Modifiers {
    let bits = modifiers.load(Ordering::Relaxed);
    let mut result = Modifiers::NONE;
    if bits & (1 << 0) != 0 {
        result = result.with(Modifiers::SHIFT);
    }
    if bits & (1 << 1) != 0 {
        result = result.with(Modifiers::CONTROL);
    }
    if bits & (1 << 2) != 0 {
        result = result.with(Modifiers::ALT);
    }
    if bits & (1 << 3) != 0 {
        result = result.with(Modifiers::META);
    }
    result
}

/// Injects remote input into the local OS through uinput. The production
/// `InputSink`.
pub struct LinuxSink {
    device: VirtualDevice,
    /// Sub-click scroll remainders, so small pixel deltas accumulate into
    /// wheel clicks instead of vanishing.
    scroll_rem_x: i32,
    scroll_rem_y: i32,
}

impl std::fmt::Debug for LinuxSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxSink").finish_non_exhaustive()
    }
}

impl LinuxSink {
    /// Creates the uinput virtual device with every key, button, and axis we
    /// can inject.
    pub fn new() -> Result<Self, LinuxInputError> {
        let mut keys = AttributeSet::<evdev::KeyCode>::new();
        for code in keymap::injectable_keys() {
            keys.insert(evdev::KeyCode(code));
        }
        for button in [0x110u16, 0x111, 0x112, 0x113, 0x114] {
            keys.insert(evdev::KeyCode(button));
        }
        let mut axes = AttributeSet::<RelativeAxisCode>::new();
        axes.insert(RelativeAxisCode::REL_X);
        axes.insert(RelativeAxisCode::REL_Y);
        axes.insert(RelativeAxisCode::REL_WHEEL);
        axes.insert(RelativeAxisCode::REL_HWHEEL);

        let device = VirtualDevice::builder()
            .map_err(LinuxInputError::Inject)?
            .name(VIRTUAL_DEVICE_NAME)
            .with_keys(&keys)
            .map_err(LinuxInputError::Inject)?
            .with_relative_axes(&axes)
            .map_err(LinuxInputError::Inject)?
            .build()
            .map_err(LinuxInputError::Inject)?;

        Ok(Self {
            device,
            scroll_rem_x: 0,
            scroll_rem_y: 0,
        })
    }

    fn emit(&mut self, events: &[evdev::InputEvent]) -> Result<(), LinuxInputError> {
        self.device.emit(events).map_err(LinuxInputError::Inject)
    }
}

impl InputSink for LinuxSink {
    type Error = LinuxInputError;

    fn inject(&mut self, event: InputEvent) -> Result<(), Self::Error> {
        match event {
            InputEvent::Key { code, action, .. } => {
                // Modifier state travels as its own key events; the wire
                // modifiers are informational here.
                let Some(evdev_code) = keymap::evdev_from_hid(code) else {
                    return Ok(()); // unmapped keys are dropped, never guessed
                };
                let value = i32::from(action == Action::Press);
                self.emit(&[evdev::InputEvent::new(EventType::KEY.0, evdev_code, value)])
            }
            InputEvent::Motion(delta) => self.emit(&[
                evdev::InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, delta.dx),
                evdev::InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, delta.dy),
            ]),
            // Absolute placement from a remote controller. A uinput relative
            // device cannot position directly, so reuse the same corner-anchored
            // stepping as an edge-crossing warp.
            InputEvent::Pointer { x, y } => self.warp(x, y),
            InputEvent::Button { button, action } => {
                let Some(code) = evdev_from_button(button) else {
                    return Ok(());
                };
                let value = i32::from(action == Action::Press);
                self.emit(&[evdev::InputEvent::new(EventType::KEY.0, code, value)])
            }
            InputEvent::Scroll(delta) => {
                self.scroll_rem_x += delta.dx;
                self.scroll_rem_y += delta.dy;
                let clicks_x = self.scroll_rem_x / PIXELS_PER_WHEEL_CLICK;
                let clicks_y = self.scroll_rem_y / PIXELS_PER_WHEEL_CLICK;
                self.scroll_rem_x -= clicks_x * PIXELS_PER_WHEEL_CLICK;
                self.scroll_rem_y -= clicks_y * PIXELS_PER_WHEEL_CLICK;
                let mut events = Vec::new();
                if clicks_y != 0 {
                    events.push(evdev::InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_WHEEL.0,
                        clicks_y,
                    ));
                }
                if clicks_x != 0 {
                    events.push(evdev::InputEvent::new(
                        EventType::RELATIVE.0,
                        RelativeAxisCode::REL_HWHEEL.0,
                        clicks_x,
                    ));
                }
                if events.is_empty() {
                    return Ok(());
                }
                self.emit(&events)
            }
        }
    }

    fn warp(&mut self, x: i32, y: i32) -> Result<(), Self::Error> {
        // A relative-only device cannot position absolutely: slam the cursor
        // into the top-left corner, then step to the requested spot.
        self.emit(&[
            evdev::InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, -65535),
            evdev::InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, -65535),
        ])?;
        self.emit(&[
            evdev::InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, x),
            evdev::InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, y),
        ])
    }
}

/// Platform-neutral aliases the Runtime wires against.
pub type OsSource = LinuxSource;
pub type OsSink = LinuxSink;

/// Reports whether the OS permissions capture and injection need are granted:
/// readable keyboards/mice in /dev/input, and a writable /dev/uinput.
pub fn diagnose() -> Vec<crate::diag::Check> {
    use crate::diag::Check;
    let mut checks = Vec::new();

    // Capture: evdev::enumerate only yields devices we could open, so compare
    // against what /dev/input actually contains to tell "no permission" from
    // "no hardware".
    let present = std::fs::read_dir("/dev/input")
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("event"))
                .count()
        })
        .unwrap_or(0);
    let mut keyboards = 0;
    let mut mice = 0;
    for (_path, device) in evdev::enumerate() {
        if device.name() == Some(VIRTUAL_DEVICE_NAME) {
            continue;
        }
        if is_keyboard(&device) {
            keyboards += 1;
        }
        if is_mouse(&device) {
            mice += 1;
        }
    }
    checks.push(if keyboards + mice > 0 {
        Check::ok(
            "input device access (capture)",
            format!("{keyboards} keyboard(s) and {mice} mouse/mice readable in /dev/input"),
        )
    } else if present > 0 {
        Check::failed(
            "input device access (capture)",
            format!(
                "{present} device(s) in /dev/input but none readable — add the user \
                 to the `input` group (`sudo usermod -aG input $USER`), then log \
                 out and back in"
            ),
        )
    } else {
        Check::failed(
            "input device access (capture)",
            "no devices in /dev/input — no input hardware visible to this system",
        )
    });

    // Injection: opening /dev/uinput for writing is exactly what the sink does.
    checks.push(
        match std::fs::OpenOptions::new().write(true).open("/dev/uinput") {
            Ok(_) => Check::ok("uinput access (injection)", "/dev/uinput is writable"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Check::failed(
                "uinput access (injection)",
                "/dev/uinput does not exist — load the module (`sudo modprobe uinput`) \
             and persist it (`echo uinput | sudo tee /etc/modules-load.d/uinput.conf`)",
            ),
            Err(e) => Check::failed(
                "uinput access (injection)",
                format!(
                    "/dev/uinput is not writable ({e}) — give the `input` group access: \
                 `echo 'KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\"' | sudo tee \
                 /etc/udev/rules.d/99-omni.rules && sudo udevadm control --reload-rules \
                 && sudo udevadm trigger`"
                ),
            ),
        },
    );

    checks
}

/// Prepares the process before any capture or screen query. The X11/Wayland
/// display server owns coordinate scaling, so there is nothing the input
/// adapter must do here; the hook exists only so the Runtime can call it on
/// every platform. (See the Windows adapter, where it declares DPI awareness.)
pub fn prepare_process() {}

/// The screen size is not discoverable from evdev (it belongs to the display
/// server); the Runtime falls back to configuration.
pub fn primary_screen_size() -> Option<(u32, u32)> {
    None
}

/// The cursor position is owned by the display server, not evdev.
pub fn cursor_position() -> Option<(i32, i32)> {
    static WARN_ONCE: std::sync::Once = std::sync::Once::new();
    let xlib = match get_xlib() {
        Some(x) => x,
        None => {
            WARN_ONCE.call_once(|| {
                tracing::warn!(
                    "Failed to open X11 display for cursor position query. If running under Wayland, cursor synchronization requires XWayland compatibility."
                );
            });
            return None;
        }
    };
    unsafe {
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            WARN_ONCE.call_once(|| {
                tracing::warn!(
                    "Failed to open X11 display for cursor position query. If running under Wayland, cursor synchronization requires XWayland compatibility."
                );
            });
            return None;
        }

        let root_window = (xlib.XDefaultRootWindow)(display);
        let mut root_return: x11_dl::xlib::Window = 0;
        let mut child_return: x11_dl::xlib::Window = 0;
        let mut root_x_return: std::os::raw::c_int = 0;
        let mut root_y_return: std::os::raw::c_int = 0;
        let mut win_x_return: std::os::raw::c_int = 0;
        let mut win_y_return: std::os::raw::c_int = 0;
        let mut mask_return: std::os::raw::c_uint = 0;

        let result = (xlib.XQueryPointer)(
            display,
            root_window,
            &mut root_return,
            &mut child_return,
            &mut root_x_return,
            &mut root_y_return,
            &mut win_x_return,
            &mut win_y_return,
            &mut mask_return,
        );

        (xlib.XCloseDisplay)(display);

        if result != 0 {
            Some((root_x_return as i32, root_y_return as i32))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x11_cursor_state_graceful_fallback() {
        // Calling hide() will either succeed (returning Some if X11 is running)
        // or fail (returning None if X11 is not running / headless / Wayland)
        // but it must NOT panic.
        let _state = X11CursorState::hide();
    }

    #[test]
    fn test_cursor_position_graceful_fallback() {
        // Calling cursor_position() should not panic on any environment.
        // It returns Some(pos) under native X11/XWayland, or None under headless/pure Wayland.
        let _pos = cursor_position();
    }
}
