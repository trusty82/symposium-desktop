//! Mise à jour automatique : au démarrage puis toutes les 6 heures, l'application
//! cherche une nouvelle version (Releases GitHub, latest.json signé), l'installe
//! et redémarre. Les mises à jour sont signées : une version non signée par la
//! clé du projet est refusée. L'addon, lui, est vérifié toutes les 10 minutes.

use crate::{addon, api, sync::Shared};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

const APP_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Une nouvelle version de l'addon (stable ou bêta) arrive dans les 10 minutes.
const ADDON_EVERY: Duration = Duration::from_secs(10 * 60);

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
    let latest = addon::latest(&settings, &api::me(&settings.site_url, &settings.token)?);
    if !addon::status(&settings, latest).update_available { return Ok(None); }
    let version = addon::install(&settings)?;
    // La fenêtre se met à jour (version installée, fil d'activité).
    let _ = app.emit("addon", &version);
    let _ = app
        .notification()
        .builder()
        .title("Symposium")
        .body(format!("Addon Symposium mis à jour en {version} : fais /reload en jeu (ou reconnecte-toi)."))
        .show();
    Ok(Some(version))
}

/// Vérifie au démarrage (après 30 secondes) : l'application toutes les 6 heures, l'addon toutes les 10 minutes.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        let mut last_app: Option<std::time::Instant> = None;
        loop {
            if last_app.map_or(true, |t| t.elapsed() >= APP_EVERY) {
                last_app = Some(std::time::Instant::now());
                if let Err(e) = tauri::async_runtime::block_on(check_and_install(&app)) {
                    eprintln!("{e}");
                }
            }
            match update_addon(&app, false) {
                Err(e) => eprintln!("{e}"),
                // Mise à jour automatique désactivée : la fenêtre signale quand même la nouvelle version.
                Ok(None) => {
                    let _ = app.emit("addon-check", ());
                }
                Ok(Some(_)) => {}
            }
            std::thread::sleep(ADDON_EVERY);
        }
    });
}
