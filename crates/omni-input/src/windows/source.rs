//! Capture: low-level keyboard and mouse hooks on a dedicated message-loop
//! thread.
//!
//! `WH_KEYBOARD_LL` and `WH_MOUSE_LL` deliver every keyboard and mouse event to
//! our callbacks before the rest of the system sees them. Each callback
//! translates the event to the protocol vocabulary and queues it for `poll`.
//! While suppressed, the callback also *swallows* the event (returns non-zero
//! instead of chaining) so the local desktop never acts on it — that is what
//! keeps input from landing on both machines while a remote session is active.
//!
//! Low-level hooks require a message loop on the thread that installed them, so
//! the hooks live on their own thread, exactly like the macOS run-loop thread.
//! Because the hook callbacks are bare C function pointers that cannot carry
//! state, the small amount of shared state (the event channel, the suppression
//! flag, the held modifiers, the last cursor point) lives in module statics;
//! only one source may exist at a time, which the daemon guarantees.

use super::{WindowsInputError, keymap};
use crate::port::InputSource;
use omni_protocol::InputEvent;
use omni_protocol::input::{Action, Modifiers, MouseButton, MouseDelta, ScrollDelta};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
    LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, PostThreadMessageW, SetCursorPos, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WHEEL_DELTA, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1,
};

use super::{PIXELS_PER_WHEEL_CLICK, screen_center};

/// How many protocol "pixels" one wheel notch carries (mirrors the Linux
/// adapter so scrolling feels the same across platforms).
const PIXELS_PER_NOTCH: i32 = PIXELS_PER_WHEEL_CLICK;

// Held-modifier bit positions, matching the `Modifiers` constants.
const MOD_SHIFT: u8 = 1 << 0;
const MOD_CTRL: u8 = 1 << 1;
const MOD_ALT: u8 = 1 << 2;
const MOD_META: u8 = 1 << 3;

// Shared state the hook callbacks reach. Single-instance, guarded by INSTALLED.
static INSTALLED: AtomicBool = AtomicBool::new(false);
static EVENT_TX: Mutex<Option<mpsc::Sender<InputEvent>>> = Mutex::new(None);
static SUPPRESSED: AtomicBool = AtomicBool::new(false);
static MODIFIERS: AtomicU8 = AtomicU8::new(0);
static LAST_X: AtomicI32 = AtomicI32::new(0);
static LAST_Y: AtomicI32 = AtomicI32::new(0);
static HAVE_LAST: AtomicBool = AtomicBool::new(false);

/// Captures local keyboard and mouse input. The production `InputSource`.
pub struct WindowsSource {
    events: mpsc::Receiver<InputEvent>,
    /// The hook thread's id, so `Drop` can ask it to quit its message loop.
    thread_id: std::sync::Arc<AtomicU32>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for WindowsSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsSource").finish_non_exhaustive()
    }
}

impl WindowsSource {
    /// Installs the low-level hooks on a dedicated thread. Fails if a source is
    /// already running or the hooks cannot be installed.
    pub fn new() -> Result<Self, WindowsInputError> {
        if INSTALLED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(WindowsInputError::AlreadyRunning);
        }

        let (event_tx, events) = mpsc::channel::<InputEvent>();
        *EVENT_TX.lock().expect("event channel lock") = Some(event_tx);
        SUPPRESSED.store(false, Ordering::Relaxed);
        MODIFIERS.store(0, Ordering::Relaxed);
        HAVE_LAST.store(false, Ordering::Relaxed);

        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), WindowsInputError>>();
        let thread_id = std::sync::Arc::new(AtomicU32::new(0));
        let thread_id_out = thread_id.clone();
        let thread = std::thread::Builder::new()
            .name("omni-input-hook".into())
            .spawn(move || run_hooks(ready_tx, thread_id_out))
            .map_err(|_| WindowsInputError::HookInstall)?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                events,
                thread_id,
                thread: Some(thread),
            }),
            _ => {
                let _ = thread.join();
                Self::release();
                Err(WindowsInputError::HookInstall)
            }
        }
    }

    /// Clears the shared state so a future source can install cleanly.
    fn release() {
        *EVENT_TX.lock().expect("event channel lock") = None;
        INSTALLED.store(false, Ordering::Release);
    }
}

impl InputSource for WindowsSource {
    type Error = WindowsInputError;

    fn poll(&mut self) -> Result<Option<InputEvent>, Self::Error> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(WindowsInputError::CaptureStopped),
        }
    }

    fn set_suppressed(&mut self, suppressed: bool) {
        let was_suppressed = SUPPRESSED.swap(suppressed, Ordering::Relaxed);
        if suppressed {
            if !was_suppressed {
                unsafe {
                    for &id in SYSTEM_CURSORS {
                        let h_cursor = create_transparent_cursor();
                        if !h_cursor.is_null() {
                            windows_sys::Win32::UI::WindowsAndMessaging::SetSystemCursor(
                                h_cursor, id,
                            );
                        }
                    }
                }
            }
            // Park the cursor at the screen centre and anchor relative motion
            // there, so deltas keep flowing instead of stalling at a screen
            // edge while we control the remote machine.
            let (cx, cy) = screen_center();
            unsafe { SetCursorPos(cx, cy) };
            LAST_X.store(cx, Ordering::Relaxed);
            LAST_Y.store(cy, Ordering::Relaxed);
            HAVE_LAST.store(true, Ordering::Relaxed);
        } else {
            if was_suppressed {
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
                        SPI_SETCURSORS,
                        0,
                        std::ptr::null_mut(),
                        SPIF_SENDCHANGE,
                    );
                }
            }
            // Re-anchor on the next real move so the cursor does not jump.
            HAVE_LAST.store(false, Ordering::Relaxed);
        }
    }
}

const SPI_SETCURSORS: u32 = 87;
const SPIF_SENDCHANGE: u32 = 2;

const SYSTEM_CURSORS: &[u32] = &[
    32512, // OCR_NORMAL
    32513, // OCR_IBEAM
    32514, // OCR_WAIT
    32515, // OCR_CROSS
    32516, // OCR_UP
    32642, // OCR_SIZENWSE
    32643, // OCR_SIZENESW
    32644, // OCR_SIZEWE
    32645, // OCR_SIZENS
    32646, // OCR_SIZEALL
    32648, // OCR_NO
    32649, // OCR_HAND
    32650, // OCR_APPSTARTING
];

unsafe fn create_transparent_cursor() -> windows_sys::Win32::UI::WindowsAndMessaging::HCURSOR {
    let and_mask = [0xFFu8; 128]; // 32 * 32 bits / 8 = 128 bytes
    let xor_mask = [0x00u8; 128];
    // Edition 2024 requires an explicit `unsafe` block even inside an `unsafe fn`.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::CreateCursor(
            std::ptr::null_mut(),
            0,
            0,
            32,
            32,
            and_mask.as_ptr() as *const std::ffi::c_void,
            xor_mask.as_ptr() as *const std::ffi::c_void,
        )
    }
}

impl Drop for WindowsSource {
    fn drop(&mut self) {
        let id = self.thread_id.load(Ordering::Acquire);
        if id != 0 {
            // Ask the hook thread's message loop to quit so it can unhook.
            unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // Restore default system cursors unconditionally
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
                SPI_SETCURSORS,
                0,
                std::ptr::null_mut(),
                SPIF_SENDCHANGE,
            );
        }
        Self::release();
    }
}

/// The body of the hook thread: install both hooks, publish readiness and this
/// thread's id, then pump messages until asked to quit.
fn run_hooks(
    ready: mpsc::Sender<Result<(), WindowsInputError>>,
    thread_id_out: std::sync::Arc<AtomicU32>,
) {
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
        if keyboard.is_null() || mouse.is_null() {
            if !keyboard.is_null() {
                UnhookWindowsHookEx(keyboard);
            }
            if !mouse.is_null() {
                UnhookWindowsHookEx(mouse);
            }
            let _ = ready.send(Err(WindowsInputError::HookInstall));
            return;
        }

        thread_id_out.store(
            windows_sys::Win32::System::Threading::GetCurrentThreadId(),
            Ordering::Release,
        );
        let _ = ready.send(Ok(()));

        let mut msg = MSG {
            hwnd: std::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT { x: 0, y: 0 },
        };
        // GetMessageW returns 0 on WM_QUIT (posted by Drop), -1 on error.
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(keyboard);
        UnhookWindowsHookEx(mouse);
    }
}

/// Queues a converted event, ignoring failures (the source may be shutting
/// down).
fn emit(event: InputEvent) {
    if let Ok(guard) = EVENT_TX.lock()
        && let Some(tx) = guard.as_ref()
    {
        let _ = tx.send(event);
    }
}

/// The current held modifiers, as the protocol type.
fn held_modifiers() -> Modifiers {
    let bits = MODIFIERS.load(Ordering::Relaxed);
    let mut result = Modifiers::NONE;
    if bits & MOD_SHIFT != 0 {
        result = result.with(Modifiers::SHIFT);
    }
    if bits & MOD_CTRL != 0 {
        result = result.with(Modifiers::CONTROL);
    }
    if bits & MOD_ALT != 0 {
        result = result.with(Modifiers::ALT);
    }
    if bits & MOD_META != 0 {
        result = result.with(Modifiers::META);
    }
    result
}

/// The held-modifier bit a virtual key controls, if it is a modifier key.
fn modifier_bit(vk: u32) -> Option<u8> {
    match vk {
        0xA0 | 0xA1 | 0x10 => Some(MOD_SHIFT), // L/R/generic Shift
        0xA2 | 0xA3 | 0x11 => Some(MOD_CTRL),  // L/R/generic Control
        0xA4 | 0xA5 | 0x12 => Some(MOD_ALT),   // L/R/generic Alt (Menu)
        0x5B | 0x5C => Some(MOD_META),         // L/R Windows
        _ => None,
    }
}

/// The low-level keyboard hook: convert and queue every key, swallow it while
/// suppressed.
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
    // Never re-capture events we (or anyone) injected.
    if info.flags & LLKHF_INJECTED == 0 {
        let message = wparam as u32;
        let action = match message {
            WM_KEYDOWN | WM_SYSKEYDOWN => Action::Press,
            WM_KEYUP | WM_SYSKEYUP => Action::Release,
            _ => return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) },
        };
        let vk = info.vkCode;
        if let Some(bit) = modifier_bit(vk) {
            match action {
                Action::Press => MODIFIERS.fetch_or(bit, Ordering::Relaxed),
                Action::Release => MODIFIERS.fetch_and(!bit, Ordering::Relaxed),
            };
        }
        if let Some(hid) = keymap::hid_from_vk(vk as u16) {
            emit(InputEvent::Key {
                code: hid,
                action,
                modifiers: held_modifiers(),
            });
        }
    }
    if SUPPRESSED.load(Ordering::Relaxed) {
        return 1; // swallow: do not pass to the local desktop
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// The low-level mouse hook: convert motion (as relative deltas), buttons, and
/// wheel; swallow everything while suppressed.
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
    // Skip injected moves (our own SendInput and the re-centring SetCursorPos).
    if info.flags & LLMHF_INJECTED == 0 {
        convert_mouse(wparam as u32, info);
    }
    if SUPPRESSED.load(Ordering::Relaxed) {
        return 1; // swallow
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// Translates one mouse event and queues it.
fn convert_mouse(message: u32, info: &MSLLHOOKSTRUCT) {
    match message {
        WM_MOUSEMOVE => {
            let (x, y) = (info.pt.x, info.pt.y);
            if !HAVE_LAST.swap(true, Ordering::Relaxed) {
                LAST_X.store(x, Ordering::Relaxed);
                LAST_Y.store(y, Ordering::Relaxed);
                return; // first sample: anchor only, no delta
            }
            let dx = x - LAST_X.load(Ordering::Relaxed);
            let dy = y - LAST_Y.load(Ordering::Relaxed);
            if dx != 0 || dy != 0 {
                emit(InputEvent::Motion(MouseDelta::new(dx, dy)));
            }
            if SUPPRESSED.load(Ordering::Relaxed) {
                // Keep the cursor parked at centre so deltas never stall.
                let (cx, cy) = screen_center();
                unsafe { SetCursorPos(cx, cy) };
                LAST_X.store(cx, Ordering::Relaxed);
                LAST_Y.store(cy, Ordering::Relaxed);
            } else {
                LAST_X.store(x, Ordering::Relaxed);
                LAST_Y.store(y, Ordering::Relaxed);
            }
        }
        WM_LBUTTONDOWN => emit_button(MouseButton::Left, Action::Press),
        WM_LBUTTONUP => emit_button(MouseButton::Left, Action::Release),
        WM_RBUTTONDOWN => emit_button(MouseButton::Right, Action::Press),
        WM_RBUTTONUP => emit_button(MouseButton::Right, Action::Release),
        WM_MBUTTONDOWN => emit_button(MouseButton::Middle, Action::Press),
        WM_MBUTTONUP => emit_button(MouseButton::Middle, Action::Release),
        WM_XBUTTONDOWN => emit_button(xbutton(info), Action::Press),
        WM_XBUTTONUP => emit_button(xbutton(info), Action::Release),
        WM_MOUSEWHEEL => {
            let notches = wheel_delta(info);
            if notches != 0 {
                emit(InputEvent::Scroll(ScrollDelta::new(
                    0,
                    notches * PIXELS_PER_NOTCH,
                )));
            }
        }
        WM_MOUSEHWHEEL => {
            let notches = wheel_delta(info);
            if notches != 0 {
                emit(InputEvent::Scroll(ScrollDelta::new(
                    notches * PIXELS_PER_NOTCH,
                    0,
                )));
            }
        }
        _ => {}
    }
}

fn emit_button(button: MouseButton, action: Action) {
    emit(InputEvent::Button { button, action });
}

/// Which extended mouse button an X-button event refers to.
fn xbutton(info: &MSLLHOOKSTRUCT) -> MouseButton {
    let which = (info.mouseData >> 16) as u16;
    if which == XBUTTON1 {
        MouseButton::Back
    } else {
        MouseButton::Forward
    }
}

/// Wheel movement in notches (positive = up / right), from the high word of
/// `mouseData`.
fn wheel_delta(info: &MSLLHOOKSTRUCT) -> i32 {
    let raw = (info.mouseData >> 16) as i16 as i32;
    raw / WHEEL_DELTA as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_transparent_cursor() {
        unsafe {
            let h_cursor = create_transparent_cursor();
            // If the cursor was successfully created (returns non-zero handle),
            // we should make sure we destroy it or restore system cursors to avoid leaks
            // in tests. But actually SetSystemCursor takes ownership and destroys it.
            // If we didn't call SetSystemCursor, we could DestroyIcon/DestroyCursor,
            // but just running SystemParametersInfoW will reset cursors.
            if !h_cursor.is_null() {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyCursor(h_cursor);
            }
            windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
                SPI_SETCURSORS,
                0,
                std::ptr::null_mut(),
                SPIF_SENDCHANGE,
            );
        }
    }
}
