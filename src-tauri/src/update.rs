//! Mise à jour automatique : au démarrage puis toutes les 6 heures, l'application
//! cherche une nouvelle version (Releases GitHub, latest.json signé), l'installe
//! et redémarre. Les mises à jour sont signées : une version non signée par la
//! clé du projet est refusée.

use crate::{addon, api, sync::Shared};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Renvoie la version installée, ou None si l'application est déjà à jour.
pub async fn check_and_install(app: &AppHandle) -> Result<Option<String>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| format!("Recherche de mise à jour impossible : {e}"))? else {
        return Ok(None);
    };
    let version = update.version.clone();
    let _ = app
        .notification()
        .builder()
        .title("Symposium")
        .body(format!("Mise à jour vers la version {version} : Symposium redémarre dans un instant."))
        .show();
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| format!("Mise à jour impossible : {e}"))?;
    // Sous Windows, l'installeur ferme et relance l'application lui-même.
    app.restart();
}

/// Met l'addon à jour s'il y a plus récent sur le site. Renvoie la version installée, s'il y en a eu une.
pub fn update_addon(app: &AppHandle, force: bool) -> Result<Option<String>, String> {
    let settings = app.state::<Shared>().lock().unwrap().clone();
    if !settings.ready() || (!force && !settings.update_addon) { return Ok(None); }
    let latest = api::me(&settings.site_url, &settings.token)?.addon_version;
    if !addon::status(&settings, latest).update_available { return Ok(None); }
    let version = addon::install(&settings)?;
    let _ = app
        .notification()
        .builder()
        .title("Symposium")
        .body(format!("Addon Symposium mis à jour en {version} : fais /reload en jeu (ou reconnecte-toi)."))
        .show();
    Ok(Some(version))
}

/// Vérifie au démarrage (après 30 secondes), puis toutes les 6 heures : l'application, puis l'addon.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            if let Err(e) = tauri::async_runtime::block_on(check_and_install(&app)) {
                eprintln!("{e}");
            }
            if let Err(e) = update_addon(&app, false) {
                eprintln!("{e}");
            }
            std::thread::sleep(EVERY);
        }
    });
}
