//! Le fichier SavedVariables\Symposium.lua écrit par l'addon (à la déconnexion
//! ou au /reload) : on y lit le dernier export du personnage.

use regex::Regex;
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}};

pub struct LastExport {
    /// Code SYMP1:… à envoyer au site.
    pub code: String,
    /// Horodatage de l'export, en secondes (heure du jeu).
    pub at: i64,
}

/// Dernier export du fichier, s'il y en a un.
pub fn read_last_export(path: &Path) -> Result<Option<LastExport>, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("Lecture de {} impossible : {e}", path.display()))?;
    let Some(start) = raw.find("[\"lastExport\"]") else { return Ok(None) };
    let block = &raw[start..];
    // Les clés à l'intérieur du JSON de l'export sont échappées (\"…\") : pas de confusion possible.
    let at = Regex::new(r#"\["at"\]\s*=\s*(\d+)"#).unwrap();
    let code = Regex::new(r#"\["code"\]\s*=\s*"(SYMP1:[A-Za-z0-9+/=]+)""#).unwrap();
    match (at.captures(block), code.captures(block)) {
        (Some(a), Some(c)) => Ok(Some(LastExport { at: a[1].parse().unwrap_or(0), code: c[1].to_string() })),
        _ => Ok(None),
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub path: String,
    /// Version du jeu (dossier _classic_beta_, _classic_…).
    pub flavor: String,
    pub account: String,
}

/// Fichiers Symposium.lua trouvés dans les emplacements habituels de World of Warcraft.
pub fn detect() -> Vec<Candidate> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "windows") {
        for drive in ['C', 'D', 'E', 'F', 'G', 'H'] {
            for dir in ["Program Files (x86)\\World of Warcraft", "Program Files\\World of Warcraft", "World of Warcraft", "Games\\World of Warcraft", "Jeux\\World of Warcraft", "Battle.net\\World of Warcraft"] {
                roots.push(PathBuf::from(format!("{drive}:\\{dir}")));
            }
        }
    } else {
        roots.push(PathBuf::from("/Applications/World of Warcraft"));
    }

    let mut found = Vec::new();
    for root in roots.into_iter().filter(|r| r.is_dir()) {
        let Ok(flavors) = fs::read_dir(&root) else { continue };
        for flavor in flavors.flatten() {
            let name = flavor.file_name().to_string_lossy().to_string();
            if !name.starts_with('_') { continue; }
            let Ok(accounts) = fs::read_dir(flavor.path().join("WTF").join("Account")) else { continue };
            for account in accounts.flatten() {
                let file = account.path().join("SavedVariables").join("Symposium.lua");
                if file.is_file() {
                    found.push(Candidate {
                        path: file.to_string_lossy().to_string(),
                        flavor: name.clone(),
                        account: account.file_name().to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_le_dernier_export() {
        let dir = std::env::temp_dir().join("symposium-desktop-test");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Symposium.lua");
        fs::write(&file, "\nSymposiumDB = {\n[\"minimapAngle\"] = 173.6,\n[\"lastExport\"] = {\n[\"at\"] = 1791290576,\n[\"json\"] = \"{\\\"at\\\":1}\",\n[\"code\"] = \"SYMP1:xV3b+/=\",\n},\n}\n").unwrap();
        let export = read_last_export(&file).unwrap().unwrap();
        assert_eq!(export.at, 1791290576);
        assert_eq!(export.code, "SYMP1:xV3b+/=");
    }

    #[test]
    fn rien_sans_export() {
        let dir = std::env::temp_dir().join("symposium-desktop-test-vide");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Symposium.lua");
        fs::write(&file, "SymposiumDB = {\n[\"minimapAngle\"] = 12,\n}\n").unwrap();
        assert!(read_last_export(&file).unwrap().is_none());
    }
}
