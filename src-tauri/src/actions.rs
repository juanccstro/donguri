//! What the squirrel can do for the user: the radial menu, asking (with the
//! current selection), quick actions, pasting into the previous app, opening
//! Claude/ChatGPT, pasting a nest as context, and the big windows.

use crate::{
    ai, clip, i18n::t, notices, popups::{self, Place, Spec}, presence, saving, store,
};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

pub const BUBBLE: &str = "bocadillo";

#[derive(Default)]
struct AskState {
    selection: Option<String>,
    quick: bool,
    /// Quick action to run as soon as the bubble opens.
    auto: Option<String>,
    /// An image to ask about (a capture or a saved image), as PNG.
    image: Option<Vec<u8>>,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(AskState::default()));
}

fn en(app: &AppHandle) -> bool {
    presence::state(app).settings.en()
}

// ------------------------------------------------------------------ menu

#[tauri::command]
pub fn open_menu(app: AppHandle) {
    // A second click on the squirrel closes it.
    if popups::is_open(&app, BUBBLE_MENU) {
        popups::close(&app, BUBBLE_MENU);
        return;
    }
    // The menu opens over an open question bubble: close the bubble.
    popups::close(&app, BUBBLE);
    popups::close(&app, crate::selection::LABEL);
    popups::open(&app, menu_spec());
}

const BUBBLE_MENU: &str = "menu";

fn menu_spec() -> Spec {
    Spec { label: BUBBLE_MENU, url: "menu", w: 460.0, h: 460.0, focusable: false, place: Place::Around }
}

/// Builds the radial menu hidden at startup, so a click shows it at once.
pub fn prepare_menu(app: &AppHandle) {
    popups::prepare(app, menu_spec());
}

#[tauri::command]
pub fn menu_action(app: AppHandle, action: String) {
    popups::close(&app, BUBBLE_MENU);
    run(&app, &action);
}

/// Shared by the radial menu, the right-click menu, the tray and shortcuts.
pub fn run(app: &AppHandle, action: &str) {
    match action {
        "ask" => ask(app, false),
        "quick" => ask(app, true),
        "kura" => open_kura(app),
        "pomodoro" => open_pomodoro(app),
        "context" => open_context_picker(app),
        "settings" => open_settings(app, None),
        "save_selection" => saving::save_selection(app),
        "save_clipboard" => saving::save_clipboard(app),
        "capture" => crate::capture::start(app, false),
        "capture_ask" => crate::capture::start(app, true),
        "clipboard" => crate::history::open(app),
        "hide" if !presence::state(app).hidden() => presence::toggle(app),
        _ => {}
    }
}

// ------------------------------------------------------------------- ask

/// Opens the question box. Grabs the selection in the foreground app first
/// (the squirrel never takes focus, so the user's app still has it).
pub fn ask(app: &AppHandle, quick: bool) {
    let exe = presence::state(app).foreground_exe.clone();
    let app = app.clone();
    presence::show_for_action(&app);
    std::thread::spawn(move || {
        let selection = clip::copy_selection(exe.as_deref());
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if quick && selection.is_none() {
                presence::animate(&handle, "error");
                notices::show(&handle, "warn", t(en(&handle), "nothing_selected"));
                return;
            }
            open_bubble(&handle, selection, quick, None);
        });
    });
}

/// Opens the question bubble with a selection already in hand (from the
/// selection acorn: read through UI Automation, no clipboard involved).
pub fn ask_with(app: &AppHandle, selection: String, auto: Option<String>) {
    presence::show_for_action(app);
    open_bubble(app, Some(selection), false, auto);
}

/// Opens the question bubble about an image (PNG bytes).
pub fn ask_about_image(app: &AppHandle, png: Vec<u8>) {
    presence::show_for_action(app);
    open_bubble_with(app, AskState { image: Some(png), ..Default::default() });
}

/// The image the open bubble is about, if any.
pub fn ask_image(app: &AppHandle) -> Option<Vec<u8>> {
    app.state::<Mutex<AskState>>().lock().ok()?.image.clone()
}

/// "Preguntar" on a saved image acorn (Kura).
#[tauri::command]
pub fn ask_acorn(app: AppHandle, id: i64) -> Result<(), String> {
    let file = saving::with_db(&app, |c| store::acorn(c, id))?
        .and_then(|a| a.file.filter(|_| a.kind == "imagen"))
        .ok_or("not an image")?;
    let png = std::fs::read(saving::data(&app).files.join(file)).map_err(|e| e.to_string())?;
    ask_about_image(&app, png);
    Ok(())
}

fn open_bubble(app: &AppHandle, selection: Option<String>, quick: bool, auto: Option<String>) {
    open_bubble_with(app, AskState { selection, quick, auto, image: None });
}

fn open_bubble_with(app: &AppHandle, state: AskState) {
    if let Ok(mut s) = app.state::<Mutex<AskState>>().lock() {
        *s = state;
    }
    popups::reopen(app, Spec { label: BUBBLE, url: "bocadillo", w: 360.0, h: 150.0, focusable: true, place: Place::Beside });
    presence::animate(app, "alert");
}

#[derive(Serialize)]
pub struct AskContext {
    selection: Option<String>,
    words: usize,
    quick: bool,
    nest: Option<store::Nest>,
    has_key: bool,
    auto: Option<String>,
    handoff: String,
    /// The image being asked about, as a data URL for the bubble to show.
    image: Option<String>,
}

#[tauri::command]
pub fn ask_context(app: AppHandle) -> AskContext {
    let (selection, quick, auto, image) = app
        .state::<Mutex<AskState>>()
        .lock()
        .map(|mut s| (s.selection.clone(), s.quick, s.auto.take(), s.image.clone()))
        .unwrap_or_default();
    let image = image.map(|png| {
        use base64::Engine;
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
    });
    let (active, handoff) = {
        let st = presence::state(&app);
        (st.settings.active_nest, st.settings.handoff.clone())
    };
    let nest = active.and_then(|id| saving::with_db(&app, |c| store::nest(c, id)).ok().flatten());
    AskContext {
        words: selection.as_deref().map_or(0, store::word_count),
        selection,
        quick,
        nest,
        has_key: ai::any_configured(),
        auto,
        handoff,
        image,
    }
}

/// No API key: open the question, with its selection and nest, in the
/// user's own Claude or ChatGPT (their subscription pays, not Donguri).
#[tauri::command]
pub fn handoff(app: AppHandle, question: String, selection: Option<String>, nest_id: Option<i64>, target: String, image: Option<bool>) {
    // Links carry only text: the image goes on the clipboard, to paste there.
    if image.unwrap_or(false)
        && let Some(png) = ask_image(&app)
    {
        let to = if target == "chatgpt" { "chatgpt" } else { "claude" };
        presence::update(&app, |s| s.handoff = to.into());
        clip::set_png(&png);
        open_link(&app, to, &question);
        notices::show(&app, "info", t(en(&app), "image_copied_paste"));
        return;
    }
    let (ctx, _) = ai::context(&app, selection.as_deref(), nest_id, Some(&question));
    let text = if ctx.trim().is_empty() { question } else { format!("{question}
{ctx}") };
    let t = if target == "chatgpt" { "chatgpt" } else { "claude" };
    presence::update(&app, |s| s.handoff = t.into());
    open_in(app, t.into(), text);
}

/// The bubble was minimised while an answer was streaming: tell the user.
pub fn answer_finished(app: &AppHandle) {
    let hidden = popups::get(app, BUBBLE).is_none_or(|w| !w.is_visible().unwrap_or(false));
    if hidden {
        notices::show(app, "answer", t(en(app), "answer_ready"));
    } else {
        presence::animate(app, "success");
    }
}

#[tauri::command]
pub fn bubble_hide(app: AppHandle) {
    if let Some(w) = popups::get(&app, BUBBLE) {
        crate::platform::set_visible(&w, false);
    }
    presence::release_peek(&app);
}

#[tauri::command]
pub fn bubble_show(app: AppHandle) {
    popups::close(&app, notices::LABEL);
    if let Some(w) = popups::get(&app, BUBBLE) {
        crate::platform::set_visible(&w, true);
        let _ = w.set_focus();
    }
}

// --------------------------------------------------------- answer actions

/// "Pegar en la app": into the app the user was in before asking.
#[tauri::command]
pub fn paste_text(app: AppHandle, text: String) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let ok = clip::paste_into_last_app(&text);
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            let msg = if ok { "pasted" } else { "copied_paste" };
            notices::show(&h, if ok { "ok" } else { "info" }, t(en(&h), msg));
        });
    });
}

#[tauri::command]
pub fn copy_text(text: String) -> bool {
    clip::set_text(&text)
}

/// "Guardar como bellota" for an answer.
#[tauri::command]
pub fn save_answer(app: AppHandle, text: String, title: Option<String>) {
    let mut p = saving::Pending {
        kind: "texto".into(),
        title: title.unwrap_or_default(),
        content: text,
        file: None,
        source: "respuesta".into(),
    };
    if p.title.trim().is_empty() {
        p.title = p.content.lines().next().unwrap_or("Respuesta").chars().take(60).collect();
    }
    saving::begin(&app, p);
}

#[cfg(windows)]
fn has_url_handler(scheme: &str) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg")
        .args(["query", &format!("HKCR\\{scheme}"), "/v", "URL Protocol"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(target_os = "macos")]
fn has_url_handler(scheme: &str) -> bool {
    scheme == "claude" && std::path::Path::new("/Applications/Claude.app").exists()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn has_url_handler(_scheme: &str) -> bool {
    false
}

/// "Abrir en Claude/ChatGPT": prefill when the target supports it, and
/// always leave the text on the clipboard as a fallback.
#[tauri::command]
pub fn open_in(app: AppHandle, target: String, text: String) {
    clip::set_text(&text);
    open_link(&app, &target, &text);
    notices::show(&app, "info", t(en(&app), "copied_paste"));
}

/// Only the desktop app gets the text in its link: in a web address it
/// would stay in the browser history (and in history sync). The web
/// versions open empty.
fn open_link(app: &AppHandle, target: &str, text: &str) {
    let url = if target == "claude" && text.chars().count() <= 6000 && has_url_handler("claude") {
        format!("claude://claude.ai/new?q={}", urlencoding::encode(text))
    } else if target == "claude" {
        "https://claude.ai/new".into()
    } else {
        "https://chatgpt.com/".into()
    };
    let _ = app.opener().open_url(url, None::<&str>);
}

// ------------------------------------------------------------- context

fn open_context_picker(app: &AppHandle) {
    presence::show_for_action(app);
    popups::open(
        app,
        Spec { label: "contexto", url: "selector?modo=contexto", w: 260.0, h: 320.0, focusable: true, place: Place::Beside },
    );
}

/// "Pegar nido": the nest pasted where the user was, either as saved
/// (`summary` false) or condensed by the AI first (`summary` true).
#[tauri::command]
pub async fn paste_context(app: AppHandle, nest_id: i64, summary: Option<bool>) -> Result<(), String> {
    popups::close(&app, "contexto");
    let (nest, text) = saving::with_db(&app, |c| Ok((store::nest(c, nest_id)?, store::nest_text(c, nest_id, 200_000, None)?)))?;
    let name = nest.map(|n| n.name).unwrap_or_default();
    let en = en(&app);
    let body = if summary.unwrap_or(false) {
        presence::animate(&app, "think");
        notices::show(&app, "info", t(en, "summarizing"));
        let prompt = format!(
            "{}

{}",
            if en {
                "Condense these saved notes into a faithful, compact summary (bullet points) that keeps every key fact, figure, name and date. Use the notes' language. Output only the summary."
            } else {
                "Condensa estas notas guardadas en un resumen fiel y compacto (viñetas) que conserve todos los datos, cifras, nombres y fechas clave. Usa el idioma de las notas. Devuelve solo el resumen."
            },
            text.text
        );
        match ai::complete(&app, &prompt).await {
            Ok(s) => {
                presence::animate(&app, "idle");
                s.trim().to_string()
            }
            Err(e) => {
                presence::animate(&app, "error");
                notices::show(&app, "warn", &e);
                presence::release_peek(&app);
                return Err(e);
            }
        }
    } else {
        if text.truncated {
            notices::show(&app, "warn", t(en, "nest_big"));
        }
        text.text
    };
    let block = if en {
        format!("Context from my \"{name}\" nest (Donguri):

{body}")
    } else {
        format!("Contexto de mi nido «{name}» (Donguri):

{body}")
    };
    presence::release_peek(&app);
    paste_text(app, block);
    Ok(())
}

// --------------------------------------------------------- big windows

/// "Salir" asks first and explains how to bring her back.
pub fn open_quit(app: &AppHandle) {
    popups::open(app, Spec { label: "salir", url: "salir", w: 420.0, h: 370.0, focusable: true, place: Place::Center });
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn autostart_get(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, on: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if on { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    Ok(al.is_enabled().unwrap_or(false))
}

pub fn open_pomodoro(app: &AppHandle) {
    popups::open(app, Spec { label: "pomodoro", url: "pomodoro", w: 340.0, h: 630.0, focusable: true, place: Place::Beside });
}

pub fn open_kura(app: &AppHandle) {
    open_kura_at(app, false);
}

/// Kura, showing the clipboard history instead of a nest when `clipboard`.
pub fn open_kura_at(app: &AppHandle, clipboard: bool) {
    let fresh = popups::get(app, "kura").is_none();
    let url = if clipboard { "kura?vista=portapapeles" } else { "kura" };
    popups::open(app, Spec { label: "kura", url, w: 460.0, h: 560.0, focusable: true, place: Place::Center });
    if clipboard && !fresh {
        let _ = app.emit_to("kura", "kura-clipboard", ());
    }
}

/// The acorn Kura should show when it opens ("Mover" in the saved notice).
static OPEN_ACORN: std::sync::Mutex<Option<i64>> = std::sync::Mutex::new(None);

/// Opens Kura on one acorn.
#[tauri::command]
pub fn open_acorn(app: AppHandle, id: i64) {
    let fresh = popups::get(&app, "kura").is_none();
    *OPEN_ACORN.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    popups::open(&app, Spec { label: "kura", url: "kura", w: 460.0, h: 560.0, focusable: true, place: Place::Center });
    if !fresh {
        let _ = app.emit_to("kura", "kura-acorn", ());
    }
}

/// Kura asks, once open, whether it should show one acorn.
#[tauri::command]
pub fn kura_take_acorn() -> Option<i64> {
    OPEN_ACORN.lock().ok()?.take()
}

pub fn open_settings(app: &AppHandle, tab: Option<&str>) {
    let fresh = popups::get(app, "ajustes").is_none();
    let url = match tab {
        Some("ia") => "ajustes?tab=ia",
        Some("integraciones") => "ajustes?tab=integraciones",
        _ => "ajustes",
    };
    popups::open(app, Spec { label: "ajustes", url, w: 560.0, h: 600.0, focusable: true, place: Place::Center });
    if !fresh && let Some(tab) = tab {
        let _ = app.emit_to("ajustes", "settings-tab", tab);
    }
}

pub fn open_welcome(app: &AppHandle) {
    popups::open(app, Spec { label: "bienvenida", url: "bienvenida", w: 540.0, h: 560.0, focusable: true, place: Place::Center });
}

#[tauri::command]
pub fn open_window(app: AppHandle, name: String) {
    match name.as_str() {
        "kura" => open_kura(&app),
        "ajustes" => open_settings(&app, None),
        "ajustes:ia" => open_settings(&app, Some("ia")),
        "ajustes:integraciones" => open_settings(&app, Some("integraciones")),
        "bienvenida" => open_welcome(&app),
        _ => {}
    }
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) {
    if url.starts_with("https://") {
        let _ = app.opener().open_url(url, None::<&str>);
    }
}
