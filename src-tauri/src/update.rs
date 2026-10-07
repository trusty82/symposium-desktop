//! Mise à jour automatique : au démarrage puis toutes les 6 heures, l'application
//! cherche une nouvelle version (Releases GitHub, latest.json signé), l'installe
//! et redémarre. Les mises à jour sont signées : une version non signée par la
//! clé du projet est refusée.

use std::time::Duration;
use tauri::AppHandle;
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

/// Vérifie au démarrage (après 30 secondes), puis toutes les 6 heures.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            if let Err(e) = tauri::async_runtime::block_on(check_and_install(&app)) {
                eprintln!("{e}");
            }
            std::thread::sleep(EVERY);
        }
    });
}
