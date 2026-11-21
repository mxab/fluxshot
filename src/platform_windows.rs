//! Windows-specific process setup winit doesn't cover early enough.

use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

/// Must run before anything touches monitors or windows.
pub fn init() {
    // Capture happens before winit's event loop exists, and winit is what
    // would otherwise opt us into per-monitor DPI awareness. Without this,
    // scaled displays report virtualized geometry and blurry captures.
    // Failure (e.g. already set via manifest) is harmless, so ignore it.
    // SAFETY: plain Win32 call with a predefined context constant.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    // Release builds use the GUI subsystem (no console flash from a hotkey);
    // reattach to the launching terminal, if any, so CLI output still shows.
    // Fails harmlessly when there is no parent console.
    // SAFETY: plain Win32 call; std opens the standard handles lazily.
    #[cfg(not(debug_assertions))]
    {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
