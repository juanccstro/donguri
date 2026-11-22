//! Saving acorns: every source (selection, area capture, drop, clipboard, an
//! answer) becomes a pending item, the nest picker asks where it goes, and
//! then the squirrel chews it.

use crate::{
    ai, clip, i18n::t, notices, popups::{self, Place, Spec}, presence, store,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager};

pub const PICKER: &str = "selector";

/// Something waiting for the user to pick a nest.
#[derive(Clone, Debug)]
pub struct Pending {
    pub kind: String,
    pub title: String,
    pub content: String,
    /// A file already copied into the app's data folder.
    pub file: Option<String>,
    pub source: String,
}

pub struct Data {
    pub db: Mutex<rusqlite::Connection>,
    pub db_path: PathBuf,
    pub files: PathBuf,
    pending: Mutex<Option<Pending>>,
}

pub fn init(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    let db_path = dir.join("donguri.db");
    let files = dir.join("bellotas");
    std::fs::create_dir_all(&files)?;
    let db = store::open(&db_path)?;
    app.manage(Data { db: Mutex::new(db), db_path, files, pending: Mutex::new(None) });
    Ok(())
}

pub fn data(app: &AppHandle) -> tauri::State<'_, Data> {
    app.state::<Data>()
}

/// Runs `f` with the database connection.
pub fn with_db<T>(app: &AppHandle, f: impl FnOnce(&rusqlite::Connection) -> store::Result<T>) -> Result<T, String> {
    let data = data(app);
    let conn = data.db.lock().unwrap_or_else(|e| e.into_inner());
    f(&conn).map_err(|e| e.to_string())
}

fn title_from(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut title: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        title.push('…');
    }
    title
}

fn stamp() -> String {
    chrono::Local::now().format("%d/%m %H:%M").to_string()
}

// ------------------------------------------------------------------ begin

/// Offers `pending` to the nest picker, unless private mode is on.
pub fn begin(app: &AppHandle, pending: Pending) {
    let (private, en) = {
        let st = presence::state(app);
        (st.settings.private_mode, st.settings.en())
    };
    if private {
        presence::animate(app, "error");
        notices::show(app, "warn", t(en, "private_on"));
        return;
    }
    *data(app).pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(pending);
    presence::show_for_action(app);
    presence::animate(app, "alert");
    popups::open(
        app,
        Spec { label: PICKER, url: "selector", w: 260.0, h: 320.0, focusable: true, place: Place::Beside },
    );
}

pub fn begin_text(app: &AppHandle, text: String, source: &str) {
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }
    begin(app, Pending { kind: "texto".into(), title: title_from(&text), content: text, file: None, source: source.into() });
}

/// Saves a PNG into the data folder and offers it.
pub fn begin_image(app: &AppHandle, png: &[u8], title: String, source: &str) {
    let name = format!("{}.png", chrono::Utc::now().timestamp_millis());
    let path = data(app).files.join(&name);
    if std::fs::write(&path, png).is_err() {
        notices::show(app, "warn", "No pude guardar la imagen");
        return;
    }
    begin(app, Pending { kind: "imagen".into(), title, content: String::new(), file: Some(name), source: source.into() });
}

pub fn rgba_to_png(width: u32, height: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let img = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// A dropped or pasted file: PDFs give their text, images are kept, text
/// files are read, anything else is kept as a file.
pub fn begin_file(app: &AppHandle, name: &str, bytes: &[u8]) {
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => {
            let png = image::load_from_memory(bytes).ok().and_then(|img| {
                let rgba = img.to_rgba8();
                rgba_to_png(rgba.width(), rgba.height(), rgba.into_raw())
            });
            match png {
                Some(png) => begin_image(app, &png, name.into(), "archivo"),
                None => notices::show(app, "warn", "Imagen no válida"),
            }
        }
        "pdf" => {
            let text = pdf_extract::extract_text_from_mem(bytes).unwrap_or_default();
            let stored = format!("{}-{}", chrono::Utc::now().timestamp_millis(), sanitize(name));
            let _ = std::fs::write(data(app).files.join(&stored), bytes);
            begin(app, Pending { kind: "pdf".into(), title: name.into(), content: text.trim().into(), file: Some(stored), source: "archivo".into() });
        }
        _ => match std::str::from_utf8(bytes) {
            Ok(text) if !text.trim().is_empty() => begin(
                app,
                Pending { kind: "texto".into(), title: name.into(), content: text.trim().into(), file: None, source: "archivo".into() },
            ),
            _ => {
                let stored = format!("{}-{}", chrono::Utc::now().timestamp_millis(), sanitize(name));
                let _ = std::fs::write(data(app).files.join(&stored), bytes);
                begin(app, Pending { kind: "archivo".into(), title: name.into(), content: String::new(), file: Some(stored), source: "archivo".into() });
            }
        },
    }
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || ".-_".contains(c) { c } else { '_' }).collect()
}

// -------------------------------------------------------------- sources

/// Alt+Shift+S: copy the selection in the foreground app and offer it.
pub fn save_selection(app: &AppHandle) {
    let exe = presence::state(app).foreground_exe.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let text = clip::copy_selection(exe.as_deref());
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || match text {
            Some(text) => begin_text(&handle, text, "seleccion"),
            None => {
                let en = presence::state(&handle).settings.en();
                presence::animate(&handle, "error");
                notices::show(&handle, "warn", t(en, "nothing_selected"));
            }
        });
    });
}

/// Ctrl+V over the squirrel or "Guardar portapapeles".
pub fn save_clipboard(app: &AppHandle) {
    match clip::read() {
        Some(clip::Content::Text(text)) => begin_text(app, text, "portapapeles"),
        Some(clip::Content::Image { width, height, rgba }) => {
            match rgba_to_png(width as u32, height as u32, rgba) {
                Some(png) => begin_image(app, &png, format!("Imagen {}", stamp()), "portapapeles"),
                None => notices::show(app, "warn", "Imagen no válida"),
            }
        }
        None => {
            let en = presence::state(app).settings.en();
            presence::animate(app, "error");
            notices::show(app, "warn", t(en, "clipboard_empty"));
        }
    }
}

// -------------------------------------------------------------- commands

#[derive(Serialize)]
pub struct PendingInfo {
    kind: String,
    title: String,
    preview: String,
    words: usize,
    /// Whether to ask the AI for a nest (opt-in, never in private mode).
    suggest: bool,
}

#[tauri::command]
pub fn pending_info(app: AppHandle) -> Option<PendingInfo> {
    let wanted = {
        let st = presence::state(&app);
        st.settings.ai_suggest && !st.settings.private_mode
    };
    let suggest = wanted && ai::any_configured();
    let data = data(&app);
    let p = data.pending.lock().unwrap_or_else(|e| e.into_inner());
    p.as_ref().map(|p| PendingInfo {
        kind: p.kind.clone(),
        title: p.title.clone(),
        preview: p.content.chars().take(160).collect(),
        words: store::word_count(&p.content),
        suggest,
    })
}

#[derive(Deserialize)]
#[serde(tag = "to", rename_all = "lowercase")]
pub enum Target {
    Nest { id: i64 },
    New { name: String },
    Unsorted,
}

#[tauri::command]
pub fn save_pending(app: AppHandle, target: Target) -> Result<(), String> {
    let Some(p) = data(&app).pending.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        popups::close(&app, PICKER);
        return Ok(());
    };
    let nest_id = match target {
        Target::Nest { id } => id,
        Target::New { name } if !name.trim().is_empty() => with_db(&app, |c| store::create_nest(c, &name, None))?,
        _ => store::UNSORTED,
    };
    let result = with_db(&app, |c| {
        store::add_acorn(
            c,
            store::NewAcorn {
                nest_id,
                kind: &p.kind,
                title: &p.title,
                content: &p.content,
                file: p.file.as_deref(),
                source: Some(&p.source),
            },
        )?;
        store::nest(c, nest_id)
    });
    popups::close(&app, PICKER);
    let en = presence::state(&app).settings.en();
    match result {
        Ok(nest) => {
            presence::animate(&app, "chew");
            let name = nest.map(|n| n.name).unwrap_or_default();
            notices::show(&app, "ok", &format!("{} {name}", t(en, "saved_in")));
            notices::changed(&app);
        }
        Err(e) => {
            presence::animate(&app, "error");
            notices::show(&app, "warn", &e);
        }
    }
    presence::release_peek(&app);
    Ok(())
}

/// Asks the AI which nest fits, if a provider is set up. Returns a nest name
/// (maybe a new one) or nothing.
#[tauri::command]
pub async fn suggest_nest(app: AppHandle) -> Option<String> {
    // Opt-in: the content leaves the computer. Never in private mode.
    {
        let st = presence::state(&app);
        if !st.settings.ai_suggest || st.settings.private_mode {
            return None;
        }
    }
    let pending = data(&app).pending.lock().ok()?.clone()?;
    let nests: Vec<String> = with_db(&app, store::list_nests)
        .ok()?
        .into_iter()
        .filter(|n| !n.system)
        .map(|n| n.name)
        .collect();
    let sample: String = if pending.content.is_empty() { pending.title.clone() } else { pending.content.chars().take(1500).collect() };
    let prompt = format!(
        "Tengo estos nidos (carpetas temáticas): {}.\nElige el más adecuado para guardar este contenido, o propone un nombre corto (1-3 palabras) para uno nuevo si ninguno encaja.\nResponde SOLO con el nombre, sin comillas ni explicación.\n\nContenido:\n{}",
        if nests.is_empty() { "(ninguno)".into() } else { nests.join(", ") },
        sample
    );
    let answer = ai::complete(&app, &prompt).await.ok()?;
    let name = answer.lines().next()?.trim().trim_matches(['"', '«', '»', '.', '*']).trim().to_string();
    (!name.is_empty() && name.chars().count() <= 40).then_some(name)
}

#[tauri::command]
pub fn save_text(app: AppHandle, text: String, source: Option<String>) {
    begin_text(&app, text, source.as_deref().unwrap_or("texto"));
}

#[tauri::command]
pub fn save_file(app: AppHandle, name: String, bytes: Vec<u8>) {
    begin_file(&app, &name, &bytes);
}

#[tauri::command]
pub fn save_clipboard_cmd(app: AppHandle) {
    save_clipboard(&app);
}

#[tauri::command]
pub fn save_selection_cmd(app: AppHandle) {
    save_selection(&app);
}

/// "Preguntar" in the nest picker: ask about the pending image instead of
/// saving it (it is not kept; the answer can be saved from the bubble).
#[tauri::command]
pub fn ask_pending(app: AppHandle) {
    let pending = data(&app).pending.lock().unwrap_or_else(|e| e.into_inner()).take();
    popups::close(&app, PICKER);
    let Some(Pending { file: Some(file), .. }) = pending.filter(|p| p.kind == "imagen") else { return };
    let path = data(&app).files.join(&file);
    let png = std::fs::read(&path).ok();
    let _ = std::fs::remove_file(&path);
    if let Some(png) = png {
        crate::actions::ask_about_image(&app, png);
    }
}
