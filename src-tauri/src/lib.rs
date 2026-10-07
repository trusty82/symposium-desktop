//! Symposium : relie l'addon WoW: Forever au site symposium-gaming.com.
//! Surveille SavedVariables\Symposium.lua et envoie la fiche du personnage au
//! site à chaque déconnexion du jeu. Tourne dans la zone de notification.

mod api;
mod savedvars;
mod settings;
mod sync;
mod update;

use settings::Settings;
use std::{path::PathBuf, sync::{Arc, Mutex}};
use sync::Shared;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State,
};
use tauri_plugin_autostart::MacosLauncher;

struct ConfigDir(PathBuf);

fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn get_settings(state: State<'_, Shared>) -> Settings {
    state.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(token: String, saved_variables: String, site_url: Option<String>, state: State<'_, Shared>, dir: State<'_, ConfigDir>) -> Result<Settings, String> {
    let mut s = state.lock().unwrap();
    if s.saved_variables != saved_variables { s.last_export_at = None; }
    s.token = token.trim().to_string();
    s.saved_variables = saved_variables.trim().to_string();
    if let Some(site) = site_url.filter(|u| !u.trim().is_empty()) { s.site_url = site.trim().trim_end_matches('/').to_string(); }
    s.save(&dir.0)?;
    Ok(s.clone())
}

#[tauri::command]
fn detect_saved_variables() -> Vec<savedvars::Candidate> {
    savedvars::detect()
}

#[tauri::command]
async fn test_connection(token: String, site_url: Option<String>, state: State<'_, Shared>) -> Result<api::Me, String> {
    let site = site_url.filter(|u| !u.trim().is_empty()).unwrap_or_else(|| state.lock().unwrap().site_url.clone());
    tauri::async_runtime::spawn_blocking(move || api::me(&site, &token)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn sync_now(app: AppHandle, state: State<'_, Shared>, dir: State<'_, ConfigDir>) -> Result<Option<settings::SyncReport>, String> {
    let (state, dir) = (state.inner().clone(), dir.0.clone());
    tauri::async_runtime::spawn_blocking(move || sync::sync_once(&app, &state, &dir, true)).await.map_err(|e| e.to_string())
}

/// Recherche une mise à jour tout de suite ; l'installe s'il y en a une.
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<String>, String> {
    update::check_and_install(&app).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Une seule instance : la relancer rouvre la fenêtre.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let state: Shared = Arc::new(Mutex::new(Settings::load(&dir)));
            app.manage(state.clone());
            app.manage(ConfigDir(dir.clone()));

            // Zone de notification : ouvrir, envoyer maintenant, quitter.
            let open = MenuItem::with_id(app, "open", "Ouvrir Symposium", true, None::<&str>)?;
            let send = MenuItem::with_id(app, "sync", "Envoyer ma fiche maintenant", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &send, &quit])?;
            let (tray_state, tray_dir) = (state.clone(), dir.clone());
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Symposium")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => show(app),
                    "sync" => {
                        let (app, state, dir) = (app.clone(), tray_state.clone(), tray_dir.clone());
                        std::thread::spawn(move || { sync::sync_once(&app, &state, &dir, true); });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        show(tray.app_handle());
                    }
                })
                .build(app)?;

            // Lancé avec Windows : discret, dans la zone de notification. Sinon, la fenêtre s'ouvre.
            if !std::env::args().any(|a| a == "--minimized") {
                show(app.handle());
            }

            sync::watch(app.handle().clone(), state, dir);
            update::watch(app.handle().clone());
            Ok(())
        })
        // Fermer la fenêtre la cache : la surveillance continue.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![get_settings, save_settings, detect_saved_variables, test_connection, sync_now, check_update])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Symposium");
}
