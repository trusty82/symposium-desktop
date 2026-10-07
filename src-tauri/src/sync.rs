//! Surveillance du fichier de l'addon : à chaque nouvel export (déconnexion ou
//! /reload en jeu), il est envoyé au site.

use crate::{api, savedvars, settings::{Settings, SyncReport}};
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}, thread, time::{Duration, SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

pub type Shared = Arc<Mutex<Settings>>;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Envoie le dernier export s'il est nouveau (ou toujours, si `force`). Renvoie le compte rendu, s'il y a eu envoi.
pub fn sync_once(app: &AppHandle, state: &Shared, config_dir: &Path, force: bool) -> Option<SyncReport> {
    let settings = state.lock().unwrap().clone();
    if !settings.ready() { return None; }

    let report = match savedvars::read_last_export(Path::new(&settings.saved_variables)) {
        Err(message) => SyncReport { at: now(), ok: false, message, character: None, url: None, export_at: None },
        Ok(None) => {
            if !force { return None; }
            SyncReport { at: now(), ok: false, message: "Pas encore d’export dans ce fichier : tape /symposium en jeu, puis déconnecte-toi ou fais /reload.".into(), character: None, url: None, export_at: None }
        }
        Ok(Some(export)) if settings.last_export_at == Some(export.at) => {
            if !force { return None; }
            // Renvoyer le même export ne changerait rien à la fiche : on l'explique plutôt.
            SyncReport {
                at: now(), ok: false,
                message: "C’est le même export qu’au dernier envoi : ta fiche est déjà à jour avec. Pour envoyer ton équipement actuel, tape /symposium en jeu puis /reload.".into(),
                character: None, url: settings.last_sync.as_ref().and_then(|r| r.url.clone()), export_at: Some(export.at),
            }
        }
        Ok(Some(export)) => match api::upload(&settings.site_url, &settings.token, &export.code) {
            Ok(done) => {
                state.lock().unwrap().last_export_at = Some(export.at);
                SyncReport {
                    at: now(), ok: true,
                    message: format!("Fiche de {} {} sur le site.", done.name, if done.created { "créée" } else { "mise à jour" }),
                    character: Some(done.name), url: Some(done.url), export_at: Some(export.at),
                }
            }
            Err(message) => SyncReport { at: now(), ok: false, message, character: None, url: None, export_at: Some(export.at) },
        },
    };

    {
        let mut s = state.lock().unwrap();
        s.last_sync = Some(report.clone());
        let _ = s.save(config_dir);
    }
    let _ = app.emit("sync", &report);
    let _ = app.notification().builder().title("Symposium").body(&report.message).show();
    Some(report)
}

/// Vérifie le fichier toutes les 5 secondes : il ne change qu'à la déconnexion ou au /reload.
pub fn watch(app: AppHandle, state: Shared, config_dir: PathBuf) {
    thread::spawn(move || {
        let mut last_seen: Option<(String, SystemTime)> = None;
        loop {
            let path = state.lock().unwrap().saved_variables.clone();
            if let Ok(modified) = std::fs::metadata(&path).and_then(|m| m.modified()) {
                let changed = last_seen.as_ref().map_or(true, |(p, t)| p != &path || *t != modified);
                if changed {
                    last_seen = Some((path, modified));
                    sync_once(&app, &state, &config_dir, false);
                }
            }
            thread::sleep(Duration::from_secs(5));
        }
    });
}
