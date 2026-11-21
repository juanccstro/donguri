//! The tiny speech bubble next to the squirrel: "saved", "answer ready",
//! warnings. Never takes focus.

use crate::{popups::{self, Place, Spec}, presence};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub const LABEL: &str = "aviso";

#[derive(Serialize, Clone, Default)]
pub struct Notice {
    /// "ok" | "warn" | "answer" | "info"
    pub kind: String,
    pub text: String,
    pub id: u64,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Notice::default()));
}

pub fn show(app: &AppHandle, kind: &str, text: &str) {
    let notice = {
        let state = app.state::<Mutex<Notice>>();
        let mut n = state.lock().unwrap_or_else(|e| e.into_inner());
        *n = Notice { kind: kind.into(), text: text.into(), id: n.id + 1 };
        n.clone()
    };
    if kind != "ok" {
        presence::animate(app, if kind == "warn" { "error" } else { "alert" });
    }
    let w = (text.chars().count() as f64 * 7.0 + 52.0).clamp(80.0, 300.0);
    popups::open(app, Spec { label: LABEL, url: "aviso", w, h: 40.0, focusable: false, place: Place::Beside });
    let _ = app.emit_to(LABEL, "notice", notice);
}

/// Tells open windows (Kura, picker) that nests or acorns changed.
pub fn changed(app: &AppHandle) {
    let _ = app.emit("data-changed", ());
}

#[tauri::command]
pub fn current_notice(app: AppHandle) -> Notice {
    app.state::<Mutex<Notice>>().lock().map(|n| n.clone()).unwrap_or_default()
}
