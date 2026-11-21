//! Windows: a low-level mouse hook (its own thread, event-driven) that
//! reports (1) presses anywhere, so non-focusable popups can close on an
//! outside click, and (2) the end of a drag or a double-click, which is when
//! text may have been selected (and almost surely was if the pointer was the
//! text I-beam).

use std::{
    cell::Cell,
    sync::{mpsc::Sender, OnceLock},
};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, POINT, WPARAM},
    System::{
        Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED},
        LibraryLoader::GetModuleHandleW,
    },
    UI::{
        Accessibility::{
            CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, UIA_TextPatternId,
        },
        Input::KeyboardAndMouse::GetDoubleClickTime,
        WindowsAndMessaging::{
            CallNextHookEx, GetCursorInfo, GetMessageW, LoadCursorW, SetWindowsHookExW, CURSORINFO,
            IDC_IBEAM, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEMOVE, WM_RBUTTONDOWN,
        },
    },
};

#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    /// A button went down somewhere on screen (physical pixels).
    Down { x: i32, y: i32 },
    /// A drag or double-click ended: text may be selected. `on_text` when
    /// the pointer was the text I-beam (then it almost surely was).
    MaybeSelection { x: i32, y: i32, on_text: bool },
}

static SENDER: OnceLock<Sender<MouseEvent>> = OnceLock::new();

thread_local! {
    /// Where and when the left button went down, and whether it was on text.
    static DOWN: Cell<(POINT, u32, bool)> = const { Cell::new((POINT { x: 0, y: 0 }, 0, false)) };
    /// The left button is down and the pointer has not been the I-beam yet.
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static LAST_UP: Cell<(POINT, u32)> = const { Cell::new((POINT { x: 0, y: 0 }, 0)) };
}

/// Drags shorter than this are clicks, not selections.
const DRAG_PX: i32 = 8;

/// Whether the pointer is the text I-beam right now (any app: the system
/// cursor handles are shared).
fn on_text() -> bool {
    static IBEAM: OnceLock<isize> = OnceLock::new();
    let ibeam = *IBEAM.get_or_init(|| unsafe { LoadCursorW(None, IDC_IBEAM).map_or(0, |c| c.0 as isize) });
    let mut info = CURSORINFO { cbSize: std::mem::size_of::<CURSORINFO>() as u32, ..Default::default() };
    unsafe { GetCursorInfo(&mut info) }.is_ok() && ibeam != 0 && info.hCursor.0 as isize == ibeam
}

fn far(a: POINT, b: POINT, px: i32) -> bool {
    (a.x - b.x).abs() > px || (a.y - b.y).abs() > px
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Keep this cheap: only read the event and hand it to another thread.
    if code >= 0
        && let Some(tx) = SENDER.get()
    {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let (pt, time) = (info.pt, info.time);
        match wparam.0 as u32 {
            WM_LBUTTONDOWN => {
                let text = on_text();
                DOWN.set((pt, time, text));
                WATCHING.set(!text);
                let _ = tx.send(MouseEvent::Down { x: pt.x, y: pt.y });
            }
            WM_RBUTTONDOWN => {
                let _ = tx.send(MouseEvent::Down { x: pt.x, y: pt.y });
            }
            // A fast drag may start before the pointer turned into the I-beam:
            // look while dragging too (only then, and only until it is seen).
            WM_MOUSEMOVE if WATCHING.get() => {
                if on_text() {
                    let (down, time, _) = DOWN.get();
                    DOWN.set((down, time, true));
                    WATCHING.set(false);
                }
            }
            WM_LBUTTONUP => {
                WATCHING.set(false);
                let (down, _, text) = DOWN.get();
                let (last, last_time) = LAST_UP.get();
                let double = !far(pt, last, 4) && time.wrapping_sub(last_time) <= unsafe { GetDoubleClickTime() };
                LAST_UP.set((pt, time));
                if far(pt, down, DRAG_PX) || double {
                    // The I-beam at the press or the release means "on text".
                    let on_text = text || on_text();
                    let _ = tx.send(MouseEvent::MaybeSelection { x: pt.x, y: pt.y, on_text });
                }
            }
            _ => {}
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Starts the hook on its own thread (it needs a message loop).
pub fn start(tx: Sender<MouseEvent>) {
    if SENDER.set(tx).is_err() {
        return;
    }
    std::thread::spawn(|| unsafe {
        let module = GetModuleHandleW(None).ok();
        let Ok(_hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(hook), module.map(Into::into), 0) else {
            return;
        };
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    });
}

/// Reads the text selected in the focused control via UI Automation, without
/// touching the clipboard. Call from one thread (COM is initialised per thread).
pub struct SelectionReader {
    uia: Option<IUIAutomation>,
}

impl SelectionReader {
    pub fn new() -> Self {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            Self { uia: CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() }
        }
    }

    /// The selection of the text control at a screen point (where the user
    /// just selected), or else of the focused one.
    pub fn selected_text(&self, at: Option<(i32, i32)>) -> Option<String> {
        let uia = self.uia.as_ref()?;
        unsafe {
            let pattern = |e: IUIAutomationElement| e.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId).ok();
            let at_point = at.and_then(|(x, y)| uia.ElementFromPoint(POINT { x, y }).ok()).and_then(pattern);
            let pattern = match at_point {
                Some(p) => p,
                None => pattern(uia.GetFocusedElement().ok()?)?,
            };
            let ranges = pattern.GetSelection().ok()?;
            if ranges.Length().ok()? < 1 {
                return None;
            }
            let text = ranges.GetElement(0).ok()?.GetText(20_000).ok()?.to_string();
            let text = text.trim().to_string();
            (text.chars().count() >= 2).then_some(text)
        }
    }
}
