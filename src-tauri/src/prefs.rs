//! Commands behind the settings window and Kura (the nest browser).

use crate::{notices, presence, saving::{self, with_db}, settings::{Mode, Settings}, shortcuts, store};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

// -------------------------------------------------------------- settings

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    presence::state(&app).settings.clone()
}

/// Changes one setting from the settings window. Returns shortcut actions
/// that failed to register, if any.
#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: Value) -> Result<Vec<String>, String> {
    let as_bool = || value.as_bool().ok_or("se esperaba sí/no");
    let as_str = || value.as_str().map(String::from).ok_or("se esperaba texto");
    match key.as_str() {
        "mode" => {
            let mode = match as_str()?.as_str() {
                "quieta" => Mode::Quieta,
                "escondida" => Mode::Escondida,
                _ => Mode::Viva,
            };
            presence::set_mode(&app, mode);
        }
        "scale" => presence::set_scale(&app, value.as_u64().unwrap_or(4) as u32),
        "fullscreen" | "capture" | "fade" | "private" => presence::set_option(&app, &key, Some(as_bool()?)),
        "palette" => {
            let v = as_str()?;
            presence::update(&app, |s| s.palette = v);
        }
        "accessory" => {
            let v = as_str()?;
            presence::update(&app, |s| s.accessory = v);
        }
        "lang" => {
            let v = as_str()?;
            presence::update(&app, |s| s.lang = if v == "en" { "en".into() } else { "es".into() });
        }
        "selection_hint" => {
            let v = as_bool()?;
            presence::update(&app, |s| s.selection_hint = v);
        }
        "update_check" => {
            let v = as_bool()?;
            presence::update(&app, |s| s.update_check = v);
        }
        "clip_history" => crate::history::set_enabled(&app, as_bool()?),
        "clip_images" => crate::history::set_images(&app, as_bool()?),
        "ai_suggest" => {
            let v = as_bool()?;
            presence::update(&app, |s| s.ai_suggest = v);
        }
        "handoff" => {
            let v = as_str()?;
            presence::update(&app, |s| s.handoff = if v == "chatgpt" { v } else { "claude".into() });
        }
        "welcome_done" => {
            let v = as_bool()?;
            presence::update(&app, |s| s.welcome_done = v);
        }
        "active_nest" => {
            let v = value.as_i64();
            presence::update(&app, |s| s.active_nest = v);
            notices::changed(&app);
        }
        k if k.starts_with("shortcut.") => {
            let v = as_str()?;
            let name = k.trim_start_matches("shortcut.").to_string();
            presence::update(&app, |s| match name.as_str() {
                "toggle" => s.shortcuts.toggle = v,
                "save" => s.shortcuts.save = v,
                "capture" => s.shortcuts.capture = v,
                "capture_ask" => s.shortcuts.capture_ask = v,
                "clipboard" => s.shortcuts.clipboard = v,
                _ => s.shortcuts.quick = v,
            });
            return Ok(shortcuts::register_all(&app));
        }
        "unblock" => presence::unblock(&app, &as_str()?),
        _ => return Err(format!("ajuste desconocido: {key}")),
    }
    crate::menus::refresh(&app);
    Ok(Vec::new())
}

#[tauri::command]
pub fn reset_shortcuts(app: AppHandle) -> Vec<String> {
    presence::update(&app, |s| s.shortcuts = Default::default());
    shortcuts::register_all(&app)
}

#[tauri::command]
pub fn open_data_folder(app: AppHandle) {
    let dir = saving::data(&app).files.parent().map(|p| p.to_path_buf());
    if let Some(dir) = dir {
        let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
    }
}

#[tauri::command]
pub fn data_folder(app: AppHandle) -> String {
    saving::data(&app).db_path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

/// Deletes every nest and acorn (the user confirmed twice in the UI).
#[tauri::command]
pub fn delete_all_data(app: AppHandle) -> Result<(), String> {
    let nests = with_db(&app, store::list_nests)?;
    for n in nests {
        let files = with_db(&app, |c| store::delete_nest(c, n.id))?;
        remove_files(&app, files);
    }
    let files = with_db(&app, |c| {
        let ids: Vec<i64> = store::list_acorns(c, store::UNSORTED)?.into_iter().map(|a| a.id).collect();
        ids.into_iter().map(|id| store::delete_acorn(c, id)).collect::<store::Result<Vec<_>>>()
    })?;
    remove_files(&app, files.into_iter().flatten().collect());
    presence::update(&app, |s| s.active_nest = None);
    notices::changed(&app);
    Ok(())
}

fn remove_files(app: &AppHandle, files: Vec<String>) {
    let dir = saving::data(app).files.clone();
    for f in files {
        let _ = std::fs::remove_file(dir.join(f));
    }
}

// ------------------------------------------------------------------ Kura

#[tauri::command]
pub fn nests(app: AppHandle) -> Result<Vec<store::Nest>, String> {
    with_db(&app, store::list_nests)
}

#[tauri::command]
pub fn acorns(app: AppHandle, nest_id: i64) -> Result<Vec<store::AcornSummary>, String> {
    with_db(&app, |c| store::list_acorns(c, nest_id))
}

#[derive(Serialize)]
pub struct AcornView {
    #[serde(flatten)]
    acorn: store::Acorn,
    /// Data URL for image acorns.
    image: Option<String>,
}

#[tauri::command]
pub fn acorn(app: AppHandle, id: i64) -> Result<Option<AcornView>, String> {
    let Some(acorn) = with_db(&app, |c| store::acorn(c, id))? else { return Ok(None) };
    let image = (acorn.kind == "imagen")
        .then_some(acorn.file.as_ref())
        .flatten()
        .and_then(|f| std::fs::read(saving::data(&app).files.join(f)).ok())
        .map(|bytes| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
    Ok(Some(AcornView { acorn, image }))
}

#[tauri::command]
pub fn search(app: AppHandle, query: String, nest_id: Option<i64>) -> Result<Vec<store::Hit>, String> {
    with_db(&app, |c| store::search(c, &query, nest_id, 60))
}

#[tauri::command]
pub fn nest_create(app: AppHandle, name: String) -> Result<i64, String> {
    let id = with_db(&app, |c| store::create_nest(c, &name, None))?;
    notices::changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn nest_rename(app: AppHandle, id: i64, name: String) -> Result<(), String> {
    with_db(&app, |c| store::rename_nest(c, id, &name))?;
    notices::changed(&app);
    Ok(())
}

#[tauri::command]
pub fn nest_color(app: AppHandle, id: i64, color: String) -> Result<(), String> {
    with_db(&app, |c| store::set_nest_color(c, id, &color))?;
    notices::changed(&app);
    Ok(())
}

#[tauri::command]
pub fn nest_delete(app: AppHandle, id: i64) -> Result<(), String> {
    let files = with_db(&app, |c| store::delete_nest(c, id))?;
    remove_files(&app, files);
    if presence::state(&app).settings.active_nest == Some(id) {
        presence::update(&app, |s| s.active_nest = None);
    }
    notices::changed(&app);
    Ok(())
}

#[tauri::command]
pub fn acorn_move(app: AppHandle, id: i64, nest_id: i64) -> Result<(), String> {
    with_db(&app, |c| store::move_acorn(c, id, nest_id))?;
    notices::changed(&app);
    Ok(())
}

/// A note written by hand in Kura.
#[tauri::command]
pub fn acorn_new(app: AppHandle, nest_id: i64, title: String, content: String) -> Result<i64, String> {
    let title = if title.trim().is_empty() { saving::title_from(&content) } else { title.trim().to_string() };
    if title.is_empty() && content.trim().is_empty() {
        return Err("empty".into());
    }
    let id = with_db(&app, |c| {
        store::add_acorn(c, store::NewAcorn { nest_id, kind: "texto", title: &title, content: &content, file: None, source: None })
    })?;
    notices::changed(&app);
    Ok(id)
}

/// Saves an acorn edited in Kura (images keep their content: only the title changes).
#[tauri::command]
pub fn acorn_edit(app: AppHandle, id: i64, title: String, content: Option<String>) -> Result<(), String> {
    with_db(&app, |c| {
        let Some(a) = store::acorn(c, id)? else { return Ok(()) };
        let content = match content {
            Some(text) if a.file.is_none() => text,
            _ => a.content,
        };
        let title = if title.trim().is_empty() { saving::title_from(&content) } else { title };
        let title = if title.trim().is_empty() { a.title } else { title };
        store::edit_acorn(c, id, &title, &content)
    })?;
    notices::changed(&app);
    Ok(())
}

#[tauri::command]
pub fn acorn_delete(app: AppHandle, id: i64) -> Result<(), String> {
    let file = with_db(&app, |c| store::delete_acorn(c, id))?;
    remove_files(&app, file.into_iter().collect());
    notices::changed(&app);
    Ok(())
}

/// Writes the nest as Markdown into Downloads and shows it in the file manager.
#[tauri::command]
pub fn nest_export(app: AppHandle, id: i64) -> Result<String, String> {
    let (md, name) = with_db(&app, |c| Ok((store::export_markdown(c, id)?, store::nest(c, id)?.map(|n| n.name))))?;
    let dir = app.path().download_dir().or_else(|_| app.path().document_dir()).map_err(|e| e.to_string())?;
    let safe: String = name
        .unwrap_or_else(|| "nido".into())
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
        .collect();
    let path = dir.join(format!("{} (Donguri).md", safe.trim()));
    std::fs::write(&path, md).map_err(|e| e.to_string())?;
    let _ = app.opener().reveal_item_in_dir(&path);
    Ok(path.to_string_lossy().into())
}
