//! Windows: everything is driven by WinEvent hooks and window messages, so the
//! app does no work while nothing changes on screen.

use crate::presence;
use std::{
    sync::{
        atomic::{AtomicIsize, AtomicU32, Ordering},
        OnceLock,
    },
    time::Duration,
};
use tauri::{AppHandle, WebviewWindow};
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL},
        System::Threading::{
            AttachThreadInput, GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW,
            PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::{
            Accessibility::{SetWinEventHook, HWINEVENTHOOK},
            Shell::{
                DefSubclassProc, SHQueryUserNotificationState, SetWindowSubclass,
                QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
            },
            WindowsAndMessaging::{
                GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow,
                BringWindowToTop, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, RegisterShellHookWindow,
                RegisterWindowMessageW, IsWindow,
                IsZoomed, SetForegroundWindow, ShowWindow,
                EVENT_SYSTEM_FOREGROUND, GWL_EXSTYLE, GWL_STYLE, OBJID_WINDOW,
                SPI_SETWORKAREA, SW_HIDE, SW_SHOWNOACTIVATE, WINEVENT_OUTOFCONTEXT,
                WINEVENT_SKIPOWNPROCESS, WM_DISPLAYCHANGE, WM_SETTINGCHANGE, WS_CAPTION, WS_EX_NOACTIVATE,
                WS_EX_TRANSPARENT,
            },
        },
    },
};

static APP: OnceLock<AppHandle> = OnceLock::new();
/// Last window of another app that had the focus (where "paste" goes).
static LAST_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Message id of shell hook notifications (RegisterShellHookWindow).
static SHELL_MSG: AtomicU32 = AtomicU32::new(0);

/// Must run on the main thread: hook callbacks are delivered to its message loop.
pub fn install(app: &AppHandle, squirrel: &WebviewWindow) {
    let _ = APP.set(app.clone());
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if let Ok(hwnd) = squirrel.hwnd() {
            let hwnd = HWND(hwnd.0);
            let _ = SetWindowSubclass(hwnd, Some(on_message), 1, 0);
            // Shell notifications (what the taskbar uses): an app going
            // fullscreen ("rude" window) or another window being activated.
            // Unlike location hooks they don't fire on every keystroke.
            SHELL_MSG.store(RegisterWindowMessageW(windows::core::w!("SHELLHOOK")), Ordering::Relaxed);
            let _ = RegisterShellHookWindow(hwnd);
        }
        let foreground = GetForegroundWindow();
        LAST_FOREGROUND.store(foreground.0 as isize, Ordering::Relaxed);
        evaluate(foreground, true);
    }
}

/// Shows without activating (never steals focus) or hides.
pub fn set_visible(win: &WebviewWindow, visible: bool) {
    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            let _ = ShowWindow(HWND(hwnd.0), if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
    }
}

unsafe extern "system" fn on_foreground(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != OBJID_WINDOW.0 {
        return;
    }
    unsafe {
        LAST_FOREGROUND.store(hwnd.0 as isize, Ordering::Relaxed);
        evaluate(hwnd, true);
    }
}

unsafe fn evaluate(hwnd: HWND, new_window: bool) {
    let Some(app) = APP.get() else { return };
    unsafe {
        let exe = exe_name(hwnd);
        let fullscreen = fullscreen_monitor(hwnd);
        let presentation = matches!(
            SHQueryUserNotificationState(),
            Ok(s) if s == QUNS_PRESENTATION_MODE || s == QUNS_RUNNING_D3D_FULL_SCREEN
        );
        presence::on_foreground(app, exe, fullscreen, presentation, new_window);
    }
}

/// Origin of the monitor `hwnd` fully covers, if it is a fullscreen window.
unsafe fn fullscreen_monitor(hwnd: HWND) -> Option<(i32, i32)> {
    unsafe {
        if hwnd.is_invalid() || hwnd == GetDesktopWindow() || hwnd == GetShellWindow() {
            return None;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if class == "WorkerW" || class == "Progman" {
            return None;
        }
        // Click-through or non-activating overlays (screen recorders, remote
        // control borders…) cover the screen without being "an app in front".
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0) != 0 {
            return None;
        }
        // A maximized window with a title bar is not fullscreen.
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        if IsZoomed(hwnd).as_bool() && style & WS_CAPTION.0 == WS_CAPTION.0 {
            return None;
        }
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).ok()?;
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
        if monitor.is_invalid() {
            return None;
        }
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let m = info.rcMonitor;
        (rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom)
            .then_some((m.left, m.top))
    }
}

/// Lower-cased executable file name of the window's process.
pub(super) unsafe fn exe_name(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        result.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_lowercase)
    }
}

/// Display added/removed or taskbar moved: re-place once things settle.
unsafe extern "system" fn on_message(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let shell = SHELL_MSG.load(Ordering::Relaxed);
    if shell != 0 && msg == shell {
        // HSHELL_WINDOWACTIVATED (4), HSHELL_RUDEAPPACTIVATED (0x8004),
        // HSHELL_FLASH/REDRAW (6/0x8006 ignored): re-check the foreground app.
        let code = wparam.0 as u32 & 0x7FFF;
        if code == 4 || code == 0x0D || code == 0x10 {
            unsafe { evaluate(GetForegroundWindow(), false) };
        }
    }
    let work_area_changed = msg == WM_SETTINGCHANGE && wparam.0 as u32 == SPI_SETWORKAREA.0;
    if (msg == WM_DISPLAYCHANGE || work_area_changed)
        && let Some(app) = APP.get()
    {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(700));
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || presence::restore(&handle));
        });
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Gives the focus back to the app the user was last in.
pub fn focus_last_app() -> bool {
    let raw = LAST_FOREGROUND.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = HWND(raw as *mut _);
    unsafe { IsWindow(Some(hwnd)).as_bool() && SetForegroundWindow(hwnd).as_bool() }
}

/// Gives keyboard focus to one of our windows the user asked for (picker,
/// chat…), even when another app is in front: Windows only lets the
/// foreground thread hand focus over, so briefly share its input state.
pub fn focus_window(win: &WebviewWindow) {
    let Ok(h) = win.hwnd() else { return };
    let hwnd = HWND(h.0);
    unsafe {
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let our_thread = GetWindowThreadProcessId(hwnd, None);
        // Sharing input with a window that is not responding (a browser that is
        // still starting…) could freeze us with it: then just ask politely.
        let hung = windows::Win32::UI::WindowsAndMessaging::IsHungAppWindow(fg).as_bool();
        let attach = fg_thread != 0 && fg_thread != our_thread && !hung;
        if attach {
            let _ = AttachThreadInput(our_thread, fg_thread, true);
        }
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attach {
            let _ = AttachThreadInput(our_thread, fg_thread, false);
        }
    }
    let _ = win.set_focus();
}


/// The top-level window at a screen point (physical px).
fn root_at(x: i32, y: i32) -> HWND {
    use windows::Win32::{
        Foundation::POINT,
        UI::WindowsAndMessaging::{GetAncestor, WindowFromPoint, GA_ROOT},
    };
    unsafe { GetAncestor(WindowFromPoint(POINT { x, y }), GA_ROOT) }
}

/// Chromium and Electron windows (browsers, VS Code, Discord…). Asking them
/// for text through UI Automation switches on their whole accessibility tree,
/// which is slow, so their selection is copied instead.
pub fn chromium_at(x: i32, y: i32) -> bool {
    unsafe {
        let mut class = [0u16; 64];
        let n = GetClassNameW(root_at(x, y), &mut class).max(0) as usize;
        String::from_utf16_lossy(&class[..n]).starts_with("Chrome_WidgetWin")
    }
}

/// Whether the window at a screen point is one of ours.
pub fn ours_at(x: i32, y: i32) -> bool {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(root_at(x, y), Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

/// Milliseconds since the last keyboard or mouse input anywhere.
pub fn idle_ms() -> u64 {
    use windows::Win32::{
        System::SystemInformation::GetTickCount,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        if !GetLastInputInfo(&mut info).as_bool() {
            return 0;
        }
        GetTickCount().wrapping_sub(info.dwTime) as u64
    }
}
