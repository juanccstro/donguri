//! Tells when the clipboard changes (Windows' clipboard format listener: no
//! polling). Copies that ask to stay out of clipboard history — password
//! managers mark theirs — are reported as private.

use std::sync::mpsc::Sender;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, HWND_MESSAGE, MSG,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSW,
};

pub enum ClipChange {
    /// Something new was copied, by this app (lower-cased exe name).
    Copied(Option<String>),
    /// Copied with "keep me out of history" (passwords and the like).
    Private,
}

static SINK: std::sync::OnceLock<std::sync::Mutex<Sender<ClipChange>>> = std::sync::OnceLock::new();

/// Starts listening on its own thread. Only the first call does anything.
pub fn start(tx: Sender<ClipChange>) {
    if SINK.set(std::sync::Mutex::new(tx)).is_err() {
        return;
    }
    std::thread::spawn(|| unsafe {
        let Ok(module) = GetModuleHandleW(None) else { return };
        let class = w!("DonguriClipboardWatch");
        let wc = WNDCLASSW { lpfnWndProc: Some(proc_), hInstance: module.into(), lpszClassName: class, ..Default::default() };
        RegisterClassW(&wc);
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(module.into()),
            None,
        ) else {
            return;
        };
        if AddClipboardFormatListener(hwnd).is_err() {
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        let change = if unsafe { private_copy() } { ClipChange::Private } else { ClipChange::Copied(unsafe { owner_exe() }) };
        if let Some(tx) = SINK.get().and_then(|m| m.lock().ok()) {
            let _ = tx.send(change);
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// The app that put the data on the clipboard (its window owns it).
unsafe fn owner_exe() -> Option<String> {
    unsafe {
        let owner = windows::Win32::System::DataExchange::GetClipboardOwner().ok()?;
        super::win32::exe_name(owner)
    }
}

/// The formats Windows documents for "don't keep this": the first one, or
/// CanIncludeInClipboardHistory / CanUploadToCloudClipboard set to 0.
unsafe fn private_copy() -> bool {
    unsafe {
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        if exclude != 0 && IsClipboardFormatAvailable(exclude).is_ok() {
            return true;
        }
        for name in [w!("CanIncludeInClipboardHistory"), w!("CanUploadToCloudClipboard")] {
            let format = RegisterClipboardFormatW(name);
            if format != 0 && IsClipboardFormatAvailable(format).is_ok() && dword_format(format) == Some(0) {
                return true;
            }
        }
        false
    }
}

unsafe fn dword_format(format: u32) -> Option<u32> {
    unsafe {
        // The app that copied may still hold the clipboard for a moment.
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !opened {
            // Can't check: be safe and treat it as private.
            return Some(0);
        }
        let value = GetClipboardData(format).ok().and_then(|h: HANDLE| {
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u32;
            let v = (!p.is_null()).then(|| *p);
            let _ = GlobalUnlock(g);
            v
        });
        let _ = CloseClipboard();
        value
    }
}
