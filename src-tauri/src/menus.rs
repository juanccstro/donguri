//! Tray icon menu and the squirrel's right-click menu (same builder).

use crate::{actions, i18n::t, presence, settings::Mode};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

const TRAY_ID: &str = "main";

fn check(app: &AppHandle, id: &str, text: &str, checked: bool) -> tauri::Result<CheckMenuItem<Wry>> {
    CheckMenuItem::with_id(app, id, text, true, checked, None::<&str>)
}

/// Pretty name for an executable: "chrome.exe" -> "chrome".
fn app_label(exe: &str) -> &str {
    exe.strip_suffix(".exe").unwrap_or(exe)
}

/// `for_squirrel` is the right-click menu: actions first, plus "hide in this
/// app", which only makes sense while another app is in front.
pub fn build(app: &AppHandle, for_squirrel: bool) -> tauri::Result<Menu<Wry>> {
    let st = presence::state(app);
    let s = st.settings.clone();
    let hidden = st.hidden();
    let foreground = st.foreground_exe.clone();
    drop(st);
    let en = s.en();
    let sc = &s.shortcuts;

    let mut menu = MenuBuilder::new(app);
    if for_squirrel {
        menu = menu
            .text("act:ask", t(en, "ask"))
            .item(&MenuItem::with_id(app, "act:save_selection", t(en, "save_selection"), true, Some(sc.save.as_str()))?)
            .item(&MenuItem::with_id(app, "act:capture", t(en, "capture"), true, Some(sc.capture.as_str()))?)
            .item(&MenuItem::with_id(app, "act:capture_ask", t(en, "capture_ask"), true, Some(sc.capture_ask.as_str()))?)
            .item(&MenuItem::with_id(app, "act:clipboard", t(en, "clipboard"), true, Some(sc.clipboard.as_str()))?)
            .text("act:save_clipboard", t(en, "save_clipboard"))
            .text("act:kura", t(en, "kura"))
            .text("act:context", t(en, "paste_context"))
            .separator();
    }
    menu = menu
        .text("toggle", t(en, if hidden { "show" } else { "hide" }))
        .text("hide_1h", t(en, "hide_1h"))
        .text("hide_tomorrow", t(en, "hide_tomorrow"));
    if for_squirrel && let Some(exe) = &foreground {
        menu = menu.text("blacklist_add", format!("{} {}", t(en, "hide_in"), app_label(exe)));
    }

    let modes = SubmenuBuilder::new(app, t(en, "mode"))
        .item(&check(app, "mode:quieta", t(en, "quieta"), s.mode == Mode::Quieta)?)
        .item(&check(app, "mode:viva", t(en, "viva"), s.mode == Mode::Viva)?)
        .item(&check(app, "mode:escondida", t(en, "escondida"), s.mode == Mode::Escondida)?)
        .build()?;

    let sizes = SubmenuBuilder::new(app, t(en, "size"))
        .item(&check(app, "scale:4", t(en, "small"), s.scale == 4)?)
        .item(&check(app, "scale:6", t(en, "medium"), s.scale == 6)?)
        .item(&check(app, "scale:8", t(en, "large"), s.scale == 8)?)
        .build()?;

    let behaviour = SubmenuBuilder::new(app, t(en, "behaviour"))
        .item(&check(app, "opt:fullscreen", t(en, "opt_fullscreen"), s.hide_fullscreen)?)
        .item(&check(app, "opt:capture", t(en, "opt_capture"), s.exclude_from_capture)?)
        .item(&check(app, "opt:fade", t(en, "opt_fade"), s.fade_when_idle)?)
        .build()?;

    let mut blocked = SubmenuBuilder::new(app, t(en, "blocked"));
    if s.blacklist.is_empty() {
        blocked = blocked.item(&MenuItem::with_id(app, "none", t(en, "no_apps"), false, None::<&str>)?);
    } else {
        for exe in &s.blacklist {
            blocked = blocked.text(format!("unblock:{exe}"), format!("{} ({})", app_label(exe), t(en, "remove")));
        }
    }
    let blocked = blocked.build()?;

    menu.separator()
        .item(&modes)
        .item(&sizes)
        .item(&behaviour)
        .item(&blocked)
        .item(&check(app, "opt:selection_hint", t(en, "quick_acorn"), s.selection_hint)?)
        .item(&check(app, "opt:private", t(en, "private"), s.private_mode)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .text("act:settings", t(en, "settings"))
        .text("quit", t(en, "quit"))
        .build()
}

pub fn init_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build(app, false)?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Donguri · clic: mostrar u ocultar · clic derecho: menú")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                presence::toggle(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    let tray = tray.build(app)?;
    #[cfg(debug_assertions)]
    #[cfg(debug_assertions)]
    eprintln!("[tray] created id={:?}", tray.id());
    // Keep the handle alive for the whole session.
    app.manage(TrayHolder(tray));
    Ok(())
}

pub struct TrayHolder(#[allow(dead_code)] tauri::tray::TrayIcon);

pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build(app, false)) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn handle(app: &AppHandle, id: &str) {
    match id {
        "toggle" => presence::toggle(app),
        "hide_1h" => presence::hide_one_hour(app),
        "hide_tomorrow" => presence::hide_until_tomorrow(app),
        "blacklist_add" => presence::blacklist_current(app),
        "quit" => actions::open_quit(app),
        _ => {
            if let Some(action) = id.strip_prefix("act:") {
                // The native menu took the foreground; give it back so the
                // action (e.g. copying the selection) targets the user's app.
                crate::platform::focus_last_app();
                actions::run(app, action);
                return;
            } else if let Some(mode) = id.strip_prefix("mode:") {
                let mode = match mode {
                    "quieta" => Mode::Quieta,
                    "escondida" => Mode::Escondida,
                    _ => Mode::Viva,
                };
                presence::set_mode(app, mode);
            } else if let Some(scale) = id.strip_prefix("scale:").and_then(|s| s.parse().ok()) {
                presence::set_scale(app, scale);
            } else if let Some(option) = id.strip_prefix("opt:") {
                presence::set_option(app, option, None);
                if option == "selection_hint" {
                    // Say what was switched and how to get it back.
                    let (on, en) = {
                        let st = presence::state(app);
                        (st.settings.selection_hint, st.settings.en())
                    };
                    crate::notices::show(app, "info", t(en, if on { "quick_on" } else { "quick_off" }));
                    if !on {
                        crate::popups::close(app, crate::selection::LABEL);
                    }
                }
            } else if let Some(exe) = id.strip_prefix("unblock:") {
                presence::unblock(app, exe);
            } else {
                return;
            }
        }
    }
    refresh(app);
}

#[tauri::command]
pub fn show_context_menu(app: AppHandle, window: tauri::Window) {
    // The pointer will move onto the menu without a "leave" event.
    crate::shortcuts::set_hover_paste(&app, false);
    if let Ok(menu) = build(&app, true) {
        let _ = window.popup_menu(&menu);
        // Showing a native menu makes us the foreground app (Windows needs it
        // to dismiss the menu properly); hand the focus back right away.
        crate::platform::focus_last_app();
    }
}
