mod actions;
mod ai;
mod capture;
mod clip;
mod history;
mod i18n;
mod integrations;
mod menus;
mod notices;
mod platform;
mod popups;
mod prefs;
mod presence;
mod saving;
mod pomodoro;
mod selection;
mod settings;
mod shortcuts;
pub mod store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just brings the existing squirrel back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if presence::state(app).hidden() {
                presence::toggle(app);
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(shortcuts::handle)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .on_menu_event(|app, event| menus::handle(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            presence::get_config,
            presence::drag_start,
            presence::drag_move,
            presence::drag_end,
            presence::drag_cancel,
            presence::walk_step,
            presence::walk_end,
            presence::peek,
            presence::unpeek,
            presence::squirrel_ready,
            presence::tab_ready,
            menus::show_context_menu,
            popups::popup_fit,
            popups::popup_tail,
            popups::popup_ready,
            popups::popup_close,
            notices::current_notice,
            shortcuts::hover_paste,
            saving::pending_info,
            saving::save_pending,
            saving::suggest_nest,
            saving::save_text,
            saving::save_file,
            saving::save_clipboard_cmd,
            saving::save_selection_cmd,
            capture::capture_image,
            capture::capture_finish,
            actions::open_menu,
            actions::menu_action,
            actions::ask_context,
            actions::bubble_hide,
            actions::bubble_show,
            actions::paste_text,
            actions::copy_text,
            actions::save_answer,
            actions::open_in,
            actions::handoff,
            actions::ask_acorn,
            saving::ask_pending,
            history::clip_history,
            history::clip_history_use,
            history::clip_history_delete,
            history::clip_history_clear,
            history::clip_history_enable,
            selection::selection_action,
            popups::popup_wanted,
            pomodoro::pomodoro_status,
            pomodoro::pomodoro_start,
            pomodoro::pomodoro_pause,
            pomodoro::pomodoro_resume,
            pomodoro::pomodoro_restart,
            pomodoro::pomodoro_skip,
            pomodoro::pomodoro_stop,
            presence::user_idle_ms,
            actions::paste_context,
            actions::open_window,
            actions::open_url,
            actions::quit_app,
            actions::autostart_get,
            actions::autostart_set,
            ai::set_key,
            ai::ai_status,
            ai::set_model,
            ai::set_model_auto,
            ai::probe_models,
            ai::set_default_provider,
            ai::test_key,
            ai::list_models,
            ai::chat_start,
            ai::chat_stop,
            integrations::integration_status,
            integrations::integration_set,
            prefs::get_settings,
            prefs::set_setting,
            prefs::reset_shortcuts,
            prefs::open_data_folder,
            prefs::data_folder,
            prefs::delete_all_data,
            prefs::nests,
            prefs::acorns,
            prefs::acorn,
            prefs::search,
            prefs::nest_create,
            prefs::nest_rename,
            prefs::nest_color,
            prefs::nest_delete,
            prefs::acorn_move,
            prefs::acorn_rename,
            prefs::acorn_delete,
            prefs::nest_export,
        ])
        .setup(|app| {
            let handle = app.handle();
            saving::init(handle)?;
            popups::init(handle);
            notices::init(handle);
            capture::init(handle);
            actions::init(handle);
            pomodoro::init(handle);
            ai::init(handle);
            presence::init(handle)?;
            menus::init_tray(handle)?;
            shortcuts::init(handle);
            selection::init(handle);
            history::init(handle);
            actions::prepare_menu(handle);
            if !presence::state(handle).settings.welcome_done {
                actions::open_welcome(handle);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
