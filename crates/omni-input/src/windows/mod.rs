//! Windows adapters for the input ports, over the Win32 low-level hooks and
//! `SendInput`.
//!
//! - [`WindowsSource`] captures keyboard and mouse events with `WH_KEYBOARD_LL`
//!   and `WH_MOUSE_LL`, and can *suppress* them (swallow them before the OS
//!   acts) while input is routed to a remote machine.
//! - [`WindowsSink`] injects events with `SendInput`, as if they came from real
//!   hardware.
//!
//! Least privilege: an ordinary desktop process may install these hooks and
//! synthesize input — no elevation and no service install. The one limit is
//! User Interface Privilege Isolation: to drive an elevated (administrator)
//! window, the daemon must itself run elevated; `diagnose` calls this out.

pub mod keymap;
mod sink;
mod source;

pub use sink::WindowsSink;
pub use source::WindowsSource;

/// Platform-neutral aliases the Runtime wires against.
pub type OsSource = WindowsSource;
pub type OsSink = WindowsSink;

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL,
};

/// How many protocol "pixels" one wheel notch carries, aligning Windows wheel
/// notches with the pixel-based scroll deltas the other platforms report.
pub(crate) const PIXELS_PER_WHEEL_CLICK: i32 = 24;

/// Why a Windows input operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsInputError {
    /// The low-level hooks could not be installed.
    HookInstall,
    /// A source is already capturing; only one may run at a time.
    AlreadyRunning,
    /// The hook thread is gone, so no more events will ever arrive.
    CaptureStopped,
    /// `SendInput` synthesized nothing — usually a more-privileged window
    /// refusing input from this (unelevated) process.
    Injection,
}

impl std::fmt::Display for WindowsInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowsInputError::HookInstall => f.write_str(
                "could not install the low-level keyboard/mouse hooks — another \
                 program may be blocking them",
            ),
            WindowsInputError::AlreadyRunning => {
                f.write_str("input capture is already running in this process")
            }
            WindowsInputError::CaptureStopped => f.write_str("input capture stopped"),
            WindowsInputError::Injection => f.write_str(
                "could not synthesize input — the focused window may require \
                 administrator rights this process does not have",
            ),
        }
    }
}

impl std::error::Error for WindowsInputError {}

/// Reports whether capture and injection can work here.
///
/// Installing low-level hooks and calling `SendInput` need no special OS
/// permission for a normal desktop process, so the check verifies a hook can
/// actually be installed and notes the one real limit (elevation, for driving
/// administrator windows).
pub fn diagnose() -> Vec<crate::diag::Check> {
    use crate::diag::Check;
    let mut checks = Vec::new();

    // Capture: prove a low-level mouse hook installs, then remove it.
    let installed = unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(noop_hook), module, 0);
        if hook.is_null() {
            false
        } else {
            UnhookWindowsHookEx(hook);
            true
        }
    };
    checks.push(if installed {
        Check::ok(
            "input hooks (capture)",
            "low-level keyboard/mouse hooks can be installed",
        )
    } else {
        Check::failed(
            "input hooks (capture)",
            "could not install a low-level mouse hook — capture will not work; \
             check for software that blocks input hooks",
        )
    });

    // Injection: always available to a desktop process; the only caveat is
    // elevation for controlling administrator windows.
    checks.push(Check::ok(
        "input injection",
        "SendInput is available; run the daemon elevated to also control \
         administrator windows (User Interface Privilege Isolation)",
    ));

    checks
}

/// A do-nothing hook used only to probe whether hooks can be installed.
unsafe extern "system" fn noop_hook(
    code: i32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// The size of the primary display in pixels — the geometry Topology builds the
/// virtual desktop from.
pub fn primary_screen_size() -> Option<(u32, u32)> {
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    if width > 0 && height > 0 {
        Some((width as u32, height as u32))
    } else {
        None
    }
}

/// Where the cursor currently is, in primary-screen pixels.
pub fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } != 0 {
        Some((point.x, point.y))
    } else {
        None
    }
}

/// The centre of the primary screen, where the cursor is parked to keep
/// relative motion flowing while controlling a remote machine.
pub(crate) fn screen_center() -> (i32, i32) {
    match primary_screen_size() {
        Some((w, h)) => (w as i32 / 2, h as i32 / 2),
        None => (0, 0),
    }
}
