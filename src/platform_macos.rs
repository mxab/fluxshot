//! macOS-specific window tweaks winit doesn't expose.

use objc2::msg_send;
use objc2::runtime::{AnyObject, Bool};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// NSScreenSaverWindowLevel: above the menu bar, Dock and other floating panels.
const OVERLAY_LEVEL: isize = 1000;
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const STATIONARY: usize = 1 << 4;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;

/// Lift the window above the menu bar and allow it over full-screen apps.
pub fn raise_overlay(window: &Window) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
        return;
    };
    // SAFETY: `ns_view` is a valid NSView owned by `window` for its lifetime, and
    // we're on the main thread (winit only hands out windows there on macOS).
    unsafe {
        let view = h.ns_view.as_ptr() as *mut AnyObject;
        let ns_window: *mut AnyObject = msg_send![view, window];
        if ns_window.is_null() {
            return;
        }
        let _: () = msg_send![ns_window, setLevel: OVERLAY_LEVEL];
        let _: () = msg_send![
            ns_window,
            setCollectionBehavior: CAN_JOIN_ALL_SPACES | STATIONARY | FULL_SCREEN_AUXILIARY
        ];
        let _: () = msg_send![ns_window, setHasShadow: Bool::NO];
    }
}
