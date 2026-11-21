//! The little acorn that appears next to the cursor after selecting text in
//! any app (Ajustes → Comportamiento can turn it off). Touching it offers
//! Preguntar · Resumir · Traducir · Explicar · Guardar for that text. It never
//! takes focus and goes away on its own, or as soon as you click elsewhere.
//!
//! Nothing is read when the acorn appears: the selected text is fetched only
//! when the user picks an action (UI Automation first, so the clipboard is
//! left alone; a copy with the clipboard restored afterwards if that fails).
//!
//! The same mouse events close the radial menu on an outside click.

use crate::{actions, clip, i18n::t, notices, popups::{self, Place, Spec}, presence, saving};
use tauri::{AppHandle, Manager};

pub const LABEL: &str = "seleccion";

/// Where the last selection ended (physical px): the acorn's text lives there.
static LAST_AT: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

/// Whether a physical point falls inside one of our windows.
fn inside(app: &AppHandle, label: &str, x: i32, y: i32) -> bool {
    let Some(w) = app.get_webview_window(label) else { return false };
    let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) else { return false };
    x >= p.x && y >= p.y && x < p.x + s.width as i32 && y < p.y + s.height as i32
}

pub fn init(app: &AppHandle) {
    // Built hidden now, so it appears the instant a selection ends.
    if presence::state(app).settings.selection_hint {
        popups::prepare(app, spec(0, 0));
    }
    #[cfg(windows)]
    start_windows(app.clone());
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
fn start_windows(app: AppHandle) {
    use crate::platform::mouse::{self, MouseEvent};
    let (tx, rx) = std::sync::mpsc::channel();
    mouse::start(tx);
    // Start UI Automation now: creating it can take longer than the moment
    // we wait for it when a selection happens.
    uia::warm();
    // Only cheap work here, so presses are handled at once.
    std::thread::spawn(move || {
        for event in rx {
            let handle = app.clone();
            match event {
                MouseEvent::Down { x, y } => {
                    let _ = app.run_on_main_thread(move || on_press(&handle, x, y));
                }
                MouseEvent::MaybeSelection { x, y, on_text } => {
                    let wanted = {
                        let st = presence::state(&app);
                        st.settings.selection_hint && !st.settings.private_mode && !st.hidden()
                    };
                    // Selections inside our own windows (Kura, bubble…) are ours.
                    if !wanted || crate::platform::ours_at(x, y) {
                        continue;
                    }
                    if on_text {
                        let _ = app.run_on_main_thread(move || show(&handle, x, y));
                    } else if !crate::platform::chromium_at(x, y) {
                        // Not sure it was text: ask the app, off this thread.
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(60));
                            if uia::selected_text(Some((x, y)), 900).is_some() {
                                let app = handle.clone();
                                let _ = handle.run_on_main_thread(move || show(&app, x, y));
                            }
                        });
                    }
                }
            }
        }
    });
}

/// A click somewhere: close the radial menu and the acorn if it was outside.
fn on_press(app: &AppHandle, x: i32, y: i32) {
    for label in ["menu", LABEL] {
        if popups::is_open(app, label) && !inside(app, label, x, y) {
            // The squirrel itself toggles the menu: let that click do it.
            if label == "menu" && presence::cursor_over_squirrel(app) {
                continue;
            }
            popups::close(app, label);
        }
    }
}

fn spec(x: i32, y: i32) -> Spec {
    Spec { label: LABEL, url: "seleccion", w: 30.0, h: 30.0, focusable: false, place: Place::At { x: x + 12, y: y + 12 } }
}

fn show(app: &AppHandle, x: i32, y: i32) {
    *LAST_AT.lock().unwrap_or_else(|e| e.into_inner()) = Some((x, y));
    popups::open(app, spec(x, y));
}

/// Reads the selection of the app in front (the acorn never takes focus, so
/// that is still the app where the text was selected). Blocks briefly.
fn read_selection(exe: Option<&str>) -> Option<String> {
    #[cfg(windows)]
    {
        let at = *LAST_AT.lock().unwrap_or_else(|e| e.into_inner());
        let chromium = at.is_some_and(|(x, y)| crate::platform::chromium_at(x, y));
        if !chromium && let Some(text) = uia::selected_text(at, 500) {
            return Some(text);
        }
    }
    clip::copy_selection(exe).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// What to do with the selected text: ask, summarize, translate, explain, save.
#[tauri::command]
pub fn selection_action(app: AppHandle, action: String) {
    popups::close(&app, LABEL);
    let exe = presence::state(&app).foreground_exe.clone();
    std::thread::spawn(move || {
        let text = read_selection(exe.as_deref());
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || match (text, action.as_str()) {
            (None, _) => {
                let en = presence::state(&handle).settings.en();
                presence::animate(&handle, "error");
                notices::show(&handle, "warn", t(en, "nothing_selected"));
            }
            (Some(text), "save") => saving::begin_text(&handle, text, "seleccion"),
            (Some(text), "ask") => actions::ask_with(&handle, text, None),
            (Some(text), other) => actions::ask_with(&handle, text, Some(other.to_string())),
        });
    });
}

/// UI Automation lives on one worker thread (COM is per thread); callers wait
/// a short moment at most, so a slow app can never stall Donguri.
#[cfg(windows)]
mod uia {
    use crate::platform::mouse::SelectionReader;
    use std::sync::{
        mpsc::{self, Sender},
        Mutex, OnceLock,
    };
    use std::time::Duration;

    type Reply = (Option<(i32, i32)>, Sender<Option<String>>);
    static WORKER: OnceLock<Mutex<Sender<Reply>>> = OnceLock::new();

    pub fn warm() {
        worker();
    }

    fn worker() -> &'static Mutex<Sender<Reply>> {
        WORKER.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Reply>();
            std::thread::spawn(move || {
                let reader = SelectionReader::new();
                for (at, reply) in rx {
                    let _ = reply.send(reader.selected_text(at));
                }
            });
            Mutex::new(tx)
        })
    }

    /// Waits at most `wait_ms`: a slow app then just gets no answer.
    pub fn selected_text(at: Option<(i32, i32)>, wait_ms: u64) -> Option<String> {
        let (tx, rx) = mpsc::channel();
        worker().lock().ok()?.send((at, tx)).ok()?;
        rx.recv_timeout(Duration::from_millis(wait_ms)).ok().flatten()
    }
}
