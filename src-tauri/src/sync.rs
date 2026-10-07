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
        Err(message) => SyncReport { at: now(), ok: false, message, character: None, url: None },
        Ok(None) => {
            if !force { return None; }
            SyncReport { at: now(), ok: false, message: "Pas encore d’export dans ce fichier : tape /symposium en jeu, puis déconnecte-toi ou fais /reload.".into(), character: None, url: None }
        }
        Ok(Some(export)) => {
            if !force && settings.last_export_at == Some(export.at) { return None; }
            match api::upload(&settings.site_url, &settings.token, &export.code) {
                Ok(done) => {
                    state.lock().unwrap().last_export_at = Some(export.at);
                    SyncReport {
                        at: now(), ok: true,
                        message: format!("Fiche de {} {} sur le site.", done.name, if done.created { "créée" } else { "mise à jour" }),
                        character: Some(done.name), url: Some(done.url),
                    }
                }
                Err(message) => SyncReport { at: now(), ok: false, message, character: None, url: None },
            }
        }
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
