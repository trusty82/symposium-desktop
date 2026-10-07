//! Réglages de l'application, gardés dans un fichier JSON du dossier de configuration.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const DEFAULT_SITE: &str = "https://symposium-gaming.com";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Adresse du site (modifiable pour les tests).
    pub site_url: String,
    /// Jeton personnel, créé sur le profil du site (symp_…).
    pub token: String,
    /// Chemin complet du fichier SavedVariables\Symposium.lua surveillé.
    pub saved_variables: String,
    /// Horodatage (jeu) du dernier export envoyé : on n'envoie pas deux fois le même.
    pub last_export_at: Option<i64>,
    /// Résultat du dernier envoi, affiché dans la fenêtre.
    pub last_sync: Option<SyncReport>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { site_url: DEFAULT_SITE.into(), token: String::new(), saved_variables: String::new(), last_export_at: None, last_sync: None }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// Secondes depuis 1970.
    pub at: i64,
    pub ok: bool,
    pub message: String,
    pub character: Option<String>,
    pub url: Option<String>,
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(dir.join("settings.json"), json).map_err(|e| e.to_string())
    }

    pub fn ready(&self) -> bool {
        !self.token.trim().is_empty() && !self.saved_variables.trim().is_empty()
    }
}
