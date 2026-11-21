//! OS integration that Tauri does not cover: showing windows without
//! activating them, foreground/fullscreen detection and display changes.

#[cfg(windows)]
pub mod mouse;
#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::{chromium_at, focus_last_app, focus_window, idle_ms, install, ours_at, set_visible};

#[cfg(not(windows))]
mod fallback {
    use tauri::{AppHandle, WebviewWindow};

    // TODO(macOS): NSWorkspace didActivateApplication for the blacklist and
    // fullscreen detection; NSApplicationDidChangeScreenParameters for displays.
    pub fn install(_app: &AppHandle, _squirrel: &WebviewWindow) {}

    /// TODO(macOS): remember the frontmost app and re-activate it.
    pub fn focus_last_app() -> bool {
        false
    }

    pub fn focus_window(win: &WebviewWindow) {
        let _ = win.set_focus();
    }

    /// TODO(macOS): CGEventSourceSecondsSinceLastEventType.
    pub fn idle_ms() -> u64 {
        0
    }

    pub fn set_visible(win: &WebviewWindow, visible: bool) {
        let _ = if visible { win.show() } else { win.hide() };
    }
}
#[cfg(not(windows))]
pub use fallback::{focus_last_app, focus_window, idle_ms, install, set_visible};
