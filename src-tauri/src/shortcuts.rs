//! Global shortcuts (configurable) and the temporary Ctrl+V while the mouse
//! is over the squirrel.

use crate::{actions, capture, presence, saving};
use std::{str::FromStr, sync::Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Default)]
pub struct Bindings(Vec<(Shortcut, &'static str)>);

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Bindings::default()));
    register_all(app);
}

/// (Re)registers the shortcuts from settings. Returns the ones that
/// could not be registered (taken by another app, invalid…).
pub fn register_all(app: &AppHandle) -> Vec<String> {
    let sc = presence::state(app).settings.shortcuts.clone();
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut bound = Vec::new();
    let mut failed = Vec::new();
    for (action, text) in [
        ("toggle", &sc.toggle),
        ("save", &sc.save),
        ("capture", &sc.capture),
        ("quick", &sc.quick),
        ("capture_ask", &sc.capture_ask),
        ("clipboard", &sc.clipboard),
    ] {
        if text.trim().is_empty() {
            continue;
        }
        match Shortcut::from_str(text) {
            Ok(s) if gs.register(s).is_ok() => bound.push((s, action)),
            _ => failed.push(action.to_string()),
        }
    }
    if let Ok(mut b) = app.state::<Mutex<Bindings>>().lock() {
        b.0 = bound;
    }
    failed
}

pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let action = app
        .state::<Mutex<Bindings>>()
        .lock()
        .ok()
        .and_then(|b| b.0.iter().find(|(s, _)| s == shortcut).map(|(_, a)| *a));
    match action {
        Some("toggle") => presence::toggle(app),
        Some("save") => saving::save_selection(app),
        Some("capture") => capture::start(app, false),
        Some("capture_ask") => capture::start(app, true),
        Some("clipboard") => crate::history::open(app),
        Some("quick") => actions::ask(app, true),
        Some("paste") => {
            if presence::cursor_over_squirrel(app) {
                saving::save_clipboard(app);
            } else {
                // The pointer left without us noticing: stop catching Ctrl+V
                // and hand this paste to the app it was meant for.
                set_hover_paste(app, false);
                crate::clip::forward_paste();
            }
        }
        _ => {}
    }
}

/// While the pointer is over the squirrel, Ctrl/Cmd+V feeds her the clipboard
/// (she never has keyboard focus, so a global shortcut is the only way).
#[tauri::command]
pub fn hover_paste(app: AppHandle, on: bool) {
    // Only while the pointer really is over her (events can arrive late).
    set_hover_paste(&app, on && presence::cursor_over_squirrel(&app));
}

pub fn set_hover_paste(app: &AppHandle, on: bool) {
    let Ok(paste) = Shortcut::from_str("CommandOrControl+V") else { return };
    let gs = app.global_shortcut();
    let bindings = app.state::<Mutex<Bindings>>();
    let Ok(mut b) = bindings.lock() else { return };
    let present = b.0.iter().any(|(s, _)| *s == paste);
    if on && !present {
        if gs.register(paste).is_ok() {
            b.0.push((paste, "paste"));
            drop(b);
            watch_hover(app.clone());
        }
    } else if !on && present {
        let _ = gs.unregister(paste);
        b.0.retain(|(s, _)| *s != paste);
    }
}

/// Ctrl+V must never stay captured by accident: while it is registered,
/// check that the pointer is still over the squirrel (leave events can be
/// missed when the cursor jumps away) and release it as soon as it is not.
/// Runs only during a hover, so it costs nothing at rest.
fn watch_hover(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let still = app
                .state::<Mutex<Bindings>>()
                .lock()
                .is_ok_and(|b| b.0.iter().any(|(_, a)| *a == "paste"));
            if !still {
                return;
            }
            if !presence::cursor_over_squirrel(&app) {
                set_hover_paste(&app, false);
                return;
            }
        }
    });
}
