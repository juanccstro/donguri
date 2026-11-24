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
    /// "saved": the acorn just saved (for Deshacer · Mover).
    pub acorn: Option<i64>,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Notice::default()));
}

pub fn show(app: &AppHandle, kind: &str, text: &str) {
    show_with(app, kind, text, None);
}

/// "Saved in …" with Deshacer · Mover for that acorn.
pub fn show_saved(app: &AppHandle, text: &str, acorn: i64) {
    show_with(app, "saved", text, Some(acorn));
}

fn show_with(app: &AppHandle, kind: &str, text: &str, acorn: Option<i64>) {
    let notice = {
        let state = app.state::<Mutex<Notice>>();
        let mut n = state.lock().unwrap_or_else(|e| e.into_inner());
        *n = Notice { kind: kind.into(), text: text.into(), id: n.id + 1, acorn };
        n.clone()
    };
    if kind != "ok" && kind != "saved" {
        presence::animate(app, if kind == "warn" { "error" } else { "alert" });
    }
    let extra = if acorn.is_some() { 150.0 } else { 0.0 };
    let w = (text.chars().count() as f64 * 7.0 + 52.0 + extra).clamp(80.0, 300.0 + extra);
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
