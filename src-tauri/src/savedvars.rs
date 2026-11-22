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
    // Le jeu n'écrit pas toujours de l'UTF-8 valide (noms lus en jeu…) : lecture tolérante,
    // le code de l'export n'utilise que des caractères ASCII.
    // Pas encore écrit : l'addon n'a jamais tourné sur ce compte (installé à l'instant par l'application).
    if !path.exists() { return Ok(None); }
    if path.is_dir() {
        return Err(format!("{} est un dossier, pas le fichier de l’addon : indique son fichier Symposium.lua (page Liaison).", path.display()));
    }
    let bytes = fs::read(path).map_err(|e| format!("Lecture de {} impossible : {e}", path.display()))?;
    let raw = String::from_utf8_lossy(&bytes);
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
    /// Le fichier existe déjà (l'addon a tourné sur ce compte) ; sinon, c'est là qu'il l'écrira.
    pub ready: bool,
}

/// Fichiers Symposium.lua sous un dossier : celui du fichier, d'un compte, d'une version
/// du jeu (_forever_…), de WTF ou de World of Warcraft.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    fn accounts(account_dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(account_dir) else { return };
        for account in entries.flatten() {
            let file = account.path().join("SavedVariables").join("Symposium.lua");
            if file.is_file() { out.push(file); }
        }
    }
    let mut out = Vec::new();
    for file in [dir.join("Symposium.lua"), dir.join("SavedVariables").join("Symposium.lua")] {
        if file.is_file() { out.push(file); }
    }
    accounts(&dir.join("Account"), &mut out);
    accounts(&dir.join("WTF").join("Account"), &mut out);
    if let Ok(flavors) = fs::read_dir(dir) {
        for flavor in flavors.flatten() {
            if flavor.file_name().to_string_lossy().starts_with('_') {
                accounts(&flavor.path().join("WTF").join("Account"), &mut out);
            }
        }
    }
    out
}

fn modified(path: &Path) -> std::time::SystemTime {
    fs::metadata(path).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
}

/// Chemin saisi ou choisi → fichier Symposium.lua. Un dossier (SavedVariables, _forever_,
/// World of Warcraft…) donne le Symposium.lua qu'il contient, le plus récent s'il y en a
/// plusieurs : lire un dossier comme un fichier échoue (« Accès refusé » sous Windows).
pub fn resolve(input: &str) -> String {
    let path = Path::new(input.trim());
    if !path.is_dir() { return input.trim().to_string(); }
    if let Some(file) = files_under(path).into_iter().max_by_key(|f| modified(f)) {
        return file.to_string_lossy().to_string();
    }
    // Dossier SavedVariables sans fichier encore (premier lancement de l'addon).
    if path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("SavedVariables")) {
        return path.join("Symposium.lua").to_string_lossy().to_string();
    }
    input.trim().to_string()
}

/// Dossiers World of Warcraft : emplacements habituels, puis à un ou deux niveaux de la
/// racine de chaque disque (D:\Jeux\World of Warcraft, D:\PROGRAMMES\WOW\World of Warcraft…).
fn wow_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "windows") {
        for drive in ['C', 'D', 'E', 'F', 'G', 'H'] {
            let base = PathBuf::from(format!("{drive}:\\"));
            if !base.is_dir() { continue; }
            for dir in ["Program Files (x86)\\World of Warcraft", "Program Files\\World of Warcraft", "World of Warcraft", "Games\\World of Warcraft", "Jeux\\World of Warcraft", "Battle.net\\World of Warcraft"] {
                roots.push(base.join(dir));
            }
            let Ok(level1) = fs::read_dir(&base) else { continue };
            for a in level1.flatten().filter(|e| e.path().is_dir()) {
                roots.push(a.path().join("World of Warcraft"));
                let Ok(level2) = fs::read_dir(a.path()) else { continue };
                for b in level2.flatten().filter(|e| e.path().is_dir()) {
                    roots.push(b.path().join("World of Warcraft"));
                }
            }
        }
    } else {
        roots.push(PathBuf::from("/Applications/World of Warcraft"));
    }
    roots.sort();
    roots.dedup();
    roots.into_iter().filter(|r| r.is_dir()).collect()
}

/// Dossiers SavedVariables des comptes du jeu (…\<version>\WTF\Account\<compte>\SavedVariables),
/// avec ou sans fichier de l'addon : ils existent dès la première connexion en jeu.
fn account_dirs_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(flavors) = fs::read_dir(root) else { return out };
    for flavor in flavors.flatten() {
        if !flavor.file_name().to_string_lossy().starts_with('_') { continue; }
        let Ok(accounts) = fs::read_dir(flavor.path().join("WTF").join("Account")) else { continue };
        for account in accounts.flatten() {
            // WTF\Account\SavedVariables : réglages communs aux comptes, pas un compte.
            if account.file_name().to_string_lossy().eq_ignore_ascii_case("SavedVariables") { continue; }
            let dir = account.path().join("SavedVariables");
            if dir.is_dir() { out.push(dir); }
        }
    }
    out
}

fn candidate(file: &Path, ready: bool) -> Candidate {
    // …\<version>\WTF\Account\<compte>\SavedVariables\Symposium.lua
    let name = |n: usize| file.ancestors().nth(n).and_then(|a| a.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Candidate { path: file.to_string_lossy().to_string(), flavor: name(5), account: name(2), ready }
}

/// Fichiers Symposium.lua trouvés dans les dossiers de World of Warcraft, puis les comptes du jeu
/// où l'addon n'a pas encore écrit (il vient d'être installé) : le meilleur choix en premier.
pub fn detect() -> Vec<Candidate> {
    detect_in(&wow_roots())
}

fn detect_in(roots: &[PathBuf]) -> Vec<Candidate> {
    let mut files: Vec<PathBuf> = roots.iter().flat_map(|r| files_under(r)).collect();
    files.sort_by_key(|f| std::cmp::Reverse(modified(f)));
    files.dedup();
    let mut empty: Vec<PathBuf> = roots.iter().flat_map(|r| account_dirs_under(r))
        .filter(|d| !d.join("Symposium.lua").is_file())
        .collect();
    // Le compte joué en dernier : son dossier SavedVariables change à chaque déconnexion.
    empty.sort_by_key(|d| std::cmp::Reverse(modified(d)));
    files.iter().map(|f| candidate(f, true))
        .chain(empty.iter().map(|d| candidate(&d.join("Symposium.lua"), false)))
        .collect()
}

/// Le fichier à surveiller sans rien demander : le plus récent, sinon le compte joué en dernier.
pub fn best() -> Option<String> {
    detect().into_iter().next().map(|c| c.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propose_un_compte_sans_fichier_apres_ceux_qui_en_ont_un() {
        let root = std::env::temp_dir().join(format!("symp-detect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let with = root.join("_classic_era_").join("WTF").join("Account").join("ANCIEN").join("SavedVariables");
        let without = root.join("_forever_").join("WTF").join("Account").join("12345#1").join("SavedVariables");
        fs::create_dir_all(&with).unwrap();
        fs::create_dir_all(&without).unwrap();
        fs::create_dir_all(root.join("_forever_").join("WTF").join("Account").join("SavedVariables")).unwrap();
        fs::write(with.join("Symposium.lua"), "SymposiumDB = {}").unwrap();

        let found = detect_in(&[root.clone()]);
        assert_eq!(found.len(), 2);
        assert!(found[0].ready);
        assert_eq!(found[0].account, "ANCIEN");
        assert!(!found[1].ready);
        assert_eq!(found[1].flavor, "_forever_");
        assert_eq!(found[1].account, "12345#1");
        assert!(found[1].path.ends_with("Symposium.lua"));
        // Pas encore écrit : rien à envoyer, sans erreur.
        assert!(read_last_export(Path::new(&found[1].path)).unwrap().is_none());
        fs::remove_dir_all(&root).unwrap();
    }

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
    fn lit_un_fichier_qui_n_est_pas_en_utf8() {
        let dir = std::env::temp_dir().join("symposium-desktop-test-latin1");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Symposium.lua");
        let mut bytes = b"SymposiumDB = {\n[\"names\"] = \"Pr\xeatre \xff\",\n[\"lastExport\"] = {\n[\"at\"] = 42,\n[\"code\"] = \"SYMP1:abc=\",\n},\n}\n".to_vec();
        bytes.push(0xC3);
        fs::write(&file, bytes).unwrap();
        let export = read_last_export(&file).unwrap().unwrap();
        assert_eq!(export.at, 42);
        assert_eq!(export.code, "SYMP1:abc=");
    }

    #[test]
    fn rien_sans_export() {
        let dir = std::env::temp_dir().join("symposium-desktop-test-vide");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Symposium.lua");
        fs::write(&file, "SymposiumDB = {\n[\"minimapAngle\"] = 12,\n}\n").unwrap();
        assert!(read_last_export(&file).unwrap().is_none());
    }

    #[test]
    fn un_dossier_donne_le_fichier_qu_il_contient() {
        let root = std::env::temp_dir().join("symposium-desktop-test-dossier");
        let _ = fs::remove_dir_all(&root);
        let sv = root.join("_forever_").join("WTF").join("Account").join("COMPTE").join("SavedVariables");
        fs::create_dir_all(&sv).unwrap();
        let file = sv.join("Symposium.lua");
        fs::write(&file, "SymposiumDB = {}\n").unwrap();
        let expected = file.to_string_lossy().to_string();
        assert_eq!(resolve(&root.to_string_lossy()), expected);
        assert_eq!(resolve(&root.join("_forever_").to_string_lossy()), expected);
        assert_eq!(resolve(&sv.to_string_lossy()), expected);
        assert_eq!(resolve(&expected), expected);
    }

    #[test]
    fn dossier_saved_variables_sans_fichier() {
        let sv = std::env::temp_dir().join("symposium-desktop-test-vide-sv").join("SavedVariables");
        let _ = fs::remove_dir_all(&sv);
        fs::create_dir_all(&sv).unwrap();
        assert_eq!(resolve(&sv.to_string_lossy()), sv.join("Symposium.lua").to_string_lossy());
    }
}
