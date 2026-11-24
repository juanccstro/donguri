//! Clipboard history (opt-in, Ajustes → Privacidad). Lives only in memory:
//! it is gone when Donguri quits, and turning it off empties it. Copies marked
//! private (password managers), copies made in private mode or in apps where
//! the squirrel hides, and Donguri's own copies are never kept.
// The clipboard watcher exists on Windows only.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::{actions, clip, i18n::t, notices, popups, presence, saving};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// It is shown inside Kura.
const LABEL: &str = "kura";
/// Entries kept, newest first.
const MAX_ITEMS: usize = 40;
/// Images among them (they take far more memory than text).
const MAX_IMAGES: usize = 8;
/// Longer copies are kept cut to this many characters.
const MAX_CHARS: usize = 100_000;

/// Password managers: never keep what is copied in them, marked or not.
const SECRET_APPS: &[&str] = &[
    "keepass.exe", "keepassxc.exe", "1password.exe", "bitwarden.exe", "lastpass.exe", "dashlane.exe",
    "keeper.exe", "enpass.exe", "proton pass.exe", "protonpass.exe", "nordpass.exe", "roboform.exe",
];

enum Body {
    Text(String),
    /// The full image (needed to paste, ask or save it) and a small
    /// thumbnail that is all the list ever shows.
    Png { full: Vec<u8>, thumb: Vec<u8> },
}

/// Thumbnails fit in this many pixels per side.
const THUMB_SIDE: u32 = 240;

fn thumbnail(png: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(png).ok()?;
    let img = if img.width() > THUMB_SIDE || img.height() > THUMB_SIDE {
        img.thumbnail(THUMB_SIDE, THUMB_SIDE)
    } else {
        img
    };
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

struct Item {
    id: u64,
    body: Body,
    at: i64,
    app: Option<String>,
}

#[derive(Default)]
pub struct History {
    items: Vec<Item>,
    next: u64,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(History::default()));
    if presence::state(app).settings.clip_history {
        start(app);
    }
}

/// Starts watching the clipboard (once; later calls do nothing).
pub fn start(app: &AppHandle) {
    #[cfg(windows)]
    {
        use crate::platform::clipwatch::{self, ClipChange};
        static STARTED: std::sync::Once = std::sync::Once::new();
        STARTED.call_once(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            clipwatch::start(tx);
            let app = app.clone();
            std::thread::spawn(move || {
                for change in rx {
                    if let ClipChange::Copied(owner) = change {
                        on_copy(&app, owner);
                    }
                }
            });
        });
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Whether copied images are kept too; turning it off forgets those already kept.
pub fn set_images(app: &AppHandle, on: bool) {
    presence::update(app, |s| s.clip_images = on);
    if !on {
        if let Ok(mut h) = app.state::<Mutex<History>>().lock() {
            h.items.retain(|i| !matches!(i.body, Body::Png { .. }));
        }
        let _ = app.emit_to(LABEL, "clip-history-changed", ());
    }
}

/// Turning it off forgets everything at once.
pub fn set_enabled(app: &AppHandle, on: bool) {
    presence::update(app, |s| s.clip_history = on);
    if on {
        start(app);
    } else {
        clip_history_clear(app.clone());
    }
}

fn on_copy(app: &AppHandle, owner: Option<String>) {
    if clip::is_ours() {
        return;
    }
    let (exe, images) = {
        let st = presence::state(app);
        if !st.settings.clip_history || st.settings.private_mode {
            return;
        }
        // The app that copied; else the one in front.
        let exe = owner.or_else(|| st.foreground_exe.clone()).filter(|e| e != "donguri.exe");
        let secret = |e: &String| st.settings.blacklist.contains(e) || SECRET_APPS.contains(&e.as_str());
        if exe.as_ref().is_some_and(secret) || st.foreground_exe.as_ref().is_some_and(secret) {
            return;
        }
        (exe, st.settings.clip_images)
    };
    // The app that copied may announce it before the data is all there.
    std::thread::sleep(std::time::Duration::from_millis(40));
    let body = match clip::read() {
        Some(clip::Content::Text(t)) => {
            if t.trim().is_empty() {
                return;
            }
            Body::Text(t.chars().take(MAX_CHARS).collect())
        }
        Some(clip::Content::Image { width, height, rgba }) => {
            if !images {
                return;
            }
            let Some(full) = saving::rgba_to_png(width as u32, height as u32, rgba) else { return };
            let Some(thumb) = thumbnail(&full) else { return };
            Body::Png { full, thumb }
        }
        None => return,
    };
    let state = app.state::<Mutex<History>>();
    let mut h = state.lock().unwrap_or_else(|e| e.into_inner());
    // Copying the same thing again moves it to the top.
    // (Some apps announce one copy twice: the same rule keeps one.)
    h.items.retain(|i| match (&i.body, &body) {
        (Body::Text(old), Body::Text(new)) => old != new,
        (Body::Png { full: old, .. }, Body::Png { full: new, .. }) => old != new,
        _ => true,
    });
    h.next += 1;
    let id = h.next;
    h.items.insert(0, Item { id, body, at: chrono::Local::now().timestamp(), app: exe });
    let mut images = 0;
    h.items.retain(|i| {
        if matches!(i.body, Body::Png { .. }) {
            images += 1;
            images <= MAX_IMAGES
        } else {
            true
        }
    });
    h.items.truncate(MAX_ITEMS);
    drop(h);
    let _ = app.emit_to(LABEL, "clip-history-changed", ());
}

#[derive(Serialize)]
pub struct Entry {
    id: u64,
    /// "text" | "image"
    kind: &'static str,
    /// Text (cut for the list) or a PNG data URL.
    preview: String,
    chars: usize,
    at: i64,
    app: Option<String>,
}

#[tauri::command]
pub fn clip_history(app: AppHandle) -> Vec<Entry> {
    use base64::Engine;
    let state = app.state::<Mutex<History>>();
    let h = state.lock().unwrap_or_else(|e| e.into_inner());
    h.items
        .iter()
        .map(|i| match &i.body {
            Body::Text(t) => Entry {
                id: i.id,
                kind: "text",
                preview: t.chars().take(400).collect(),
                chars: t.chars().count(),
                at: i.at,
                app: i.app.clone(),
            },
            Body::Png { thumb, .. } => Entry {
                id: i.id,
                kind: "image",
                preview: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(thumb)),
                chars: 0,
                at: i.at,
                app: i.app.clone(),
            },
        })
        .collect()
}

fn take_body(app: &AppHandle, id: u64) -> Option<Body> {
    let state = app.state::<Mutex<History>>();
    let h = state.lock().ok()?;
    h.items.iter().find(|i| i.id == id).map(|i| match &i.body {
        Body::Text(t) => Body::Text(t.clone()),
        Body::Png { full, .. } => Body::Png { full: full.clone(), thumb: Vec::new() },
    })
}

/// paste · ask · save · copy for one entry.
#[tauri::command]
pub fn clip_history_use(app: AppHandle, id: u64, action: String) {
    let Some(body) = take_body(&app, id) else { return };
    popups::close(&app, LABEL);
    match (action.as_str(), body) {
        ("paste", Body::Text(text)) => actions::paste_text(app, text),
        ("paste", Body::Png { full: png, .. }) => {
            std::thread::spawn(move || {
                let ok = clip::paste_png_into_last_app(&png);
                let h = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let en = presence::state(&h).settings.en();
                    notices::show(&h, if ok { "ok" } else { "info" }, t(en, if ok { "pasted" } else { "copied_paste" }));
                });
            });
        }
        ("ask", Body::Text(text)) => actions::ask_with(&app, text, None),
        ("ask", Body::Png { full: png, .. }) => actions::ask_about_image(&app, png),
        ("save", Body::Text(text)) => saving::begin_text(&app, text, "portapapeles"),
        ("save", Body::Png { full: png, .. }) => {
            let title = format!("Imagen {}", chrono::Local::now().format("%d/%m %H:%M"));
            saving::begin_image(&app, &png, title, "portapapeles");
        }
        (_, Body::Text(text)) => {
            clip::set_text(&text);
        }
        (_, Body::Png { full: png, .. }) => {
            clip::set_png(&png);
        }
    }
}

#[tauri::command]
pub fn clip_history_delete(app: AppHandle, id: u64) {
    if let Ok(mut h) = app.state::<Mutex<History>>().lock() {
        h.items.retain(|i| i.id != id);
    }
}

#[tauri::command]
pub fn clip_history_clear(app: AppHandle) {
    if let Ok(mut h) = app.state::<Mutex<History>>().lock() {
        h.items.clear();
    }
    let _ = app.emit_to(LABEL, "clip-history-changed", ());
}

#[tauri::command]
pub fn clip_history_enable(app: AppHandle, on: bool) {
    set_enabled(&app, on);
    let _ = app.emit_to(LABEL, "clip-history-changed", ());
}

pub fn open(app: &AppHandle) {
    actions::open_kura_at(app, true);
}

#[cfg(test)]
mod tests {
    use super::{thumbnail, THUMB_SIDE};

    #[test]
    fn thumbnails_are_small() {
        let shot = image::RgbaImage::from_pixel(1920, 1080, image::Rgba([200, 120, 80, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(shot).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let png = png.into_inner();
        let thumb = image::load_from_memory(&thumbnail(&png).unwrap()).unwrap();
        assert!(thumb.width() <= THUMB_SIDE && thumb.height() <= THUMB_SIDE);
        assert_eq!(thumb.width(), THUMB_SIDE);
    }
}
