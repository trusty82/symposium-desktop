//! Mise à jour de l'addon Symposium dans le dossier du jeu, depuis le site
//! (version stable). Le dossier est déduit du fichier SavedVariables choisi :
//! …\<version du jeu>\WTF\Account\<compte>\SavedVariables\Symposium.lua
//!   → …\<version du jeu>\Interface\AddOns\Symposium

use crate::{api, settings::Settings};
use serde::Serialize;
use std::{cmp::Ordering, fs, io::Cursor, path::{Path, PathBuf}};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AddonStatus {
    pub dir: Option<String>,
    pub installed: Option<String>,
    pub latest: Option<String>,
    pub update_available: bool,
}

/// Dossier Interface\AddOns\Symposium correspondant au fichier SavedVariables.
pub fn addon_dir(saved_variables: &Path) -> Option<PathBuf> {
    // SavedVariables → <compte> → Account → WTF → <version du jeu>
    let flavor = saved_variables.parent()?.parent()?.parent()?.parent()?.parent()?;
    Some(flavor.join("Interface").join("AddOns").join("Symposium"))
}

/// Version lue dans Symposium.toc (« ## Version: 0.9.1 »).
pub fn installed_version(dir: &Path) -> Option<String> {
    let toc = fs::read_to_string(dir.join("Symposium.toc")).ok()?;
    toc.lines().find_map(|l| l.trim().strip_prefix("## Version:").map(|v| v.trim().to_string()))
}

/// Compare « 0.10.0 » à « 0.9.2-beta1 » : nombre par nombre ; à nombres égaux, une bêta passe avant la stable.
pub fn compare(a: &str, b: &str) -> Ordering {
    let split = |v: &str| {
        let (num, pre) = v.split_once('-').map_or((v, None), |(n, p)| (n, Some(p.to_string())));
        (num.split('.').map(|n| n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>(), pre)
    };
    let ((na, pa), (nb, pb)) = (split(a), split(b));
    for i in 0..na.len().max(nb.len()) {
        match na.get(i).unwrap_or(&0).cmp(nb.get(i).unwrap_or(&0)) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    match (pa, pb) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(x), Some(y)) => x.cmp(&y),
    }
}

pub fn status(settings: &Settings, latest: Option<String>) -> AddonStatus {
    let dir = addon_dir(Path::new(&settings.saved_variables));
    let installed = dir.as_deref().and_then(installed_version);
    let update_available = match (&installed, &latest) {
        (Some(i), Some(l)) => compare(l, i) == Ordering::Greater,
        (None, Some(_)) => dir.as_ref().is_some_and(|d| d.parent().is_some_and(Path::is_dir)),
        _ => false,
    };
    AddonStatus { dir: dir.map(|d| d.to_string_lossy().to_string()), installed, latest, update_available }
}

/// Télécharge et installe l'addon du site. L'ancien dossier est gardé à côté (.Symposium-ancien).
pub fn install(settings: &Settings) -> Result<String, String> {
    let dir = addon_dir(Path::new(&settings.saved_variables)).ok_or("Dossier de l’addon introuvable : choisis d’abord ton fichier Symposium.lua.")?;
    let addons = dir.parent().ok_or("Dossier AddOns introuvable.")?;
    fs::create_dir_all(addons).map_err(|e| format!("Dossier AddOns inaccessible : {e}"))?;

    let bytes = api::download_addon(&settings.site_url, &settings.token)?;
    install_zip(&dir, bytes)
}

/// Installe le zip (dossier Symposium/…) à la place de `dir`.
pub fn install_zip(dir: &Path, bytes: Vec<u8>) -> Result<String, String> {
    let addons = dir.parent().ok_or("Dossier AddOns introuvable.")?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Zip de l’addon illisible : {e}"))?;

    // Extraction à côté, puis échange des dossiers : jamais d'addon à moitié copié.
    let staging = addons.join(".Symposium-maj");
    let _ = fs::remove_dir_all(&staging);
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = file.enclosed_name() else { continue };
        let Ok(relative) = name.strip_prefix("Symposium") else { continue };
        let target = staging.join(relative);
        if file.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            let mut out = fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        }
    }
    let version = installed_version(&staging).ok_or("Le zip de l’addon ne contient pas Symposium.toc.")?;

    let backup = addons.join(".Symposium-ancien");
    let _ = fs::remove_dir_all(&backup);
    if dir.exists() {
        fs::rename(dir, &backup).map_err(|e| format!("Impossible de remplacer l’addon (fichier ouvert ?) : {e}"))?;
    }
    if let Err(e) = fs::rename(&staging, dir) {
        let _ = fs::rename(&backup, dir);
        return Err(format!("Installation de l’addon impossible : {e}"));
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_les_versions() {
        assert_eq!(compare("0.10.0", "0.9.2"), Ordering::Greater);
        assert_eq!(compare("0.9.1", "0.9.1"), Ordering::Equal);
        assert_eq!(compare("0.9.2", "0.9.2-beta1"), Ordering::Greater);
        assert_eq!(compare("0.9.1", "0.9.2-beta1"), Ordering::Less);
    }

    #[test]
    fn installe_le_zip_en_gardant_l_ancien() {
        use std::io::Write;
        let root = std::env::temp_dir().join("symposium-desktop-addon-test");
        let _ = fs::remove_dir_all(&root);
        let dir = root.join("Interface").join("AddOns").join("Symposium");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Symposium.toc"), "## Version: 0.9.0\n").unwrap();
        fs::write(dir.join("Ancien.lua"), "-- supprimé dans la nouvelle version").unwrap();

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("Symposium/Symposium.toc", options).unwrap();
            zip.write_all(b"## Title: Symposium\n## Version: 0.9.2\n").unwrap();
            zip.start_file("Symposium/Epgp.lua", options).unwrap();
            zip.write_all(b"-- nouveau").unwrap();
            zip.finish().unwrap();
        }

        assert_eq!(install_zip(&dir, buffer.into_inner()).unwrap(), "0.9.2");
        assert_eq!(installed_version(&dir).as_deref(), Some("0.9.2"));
        assert!(dir.join("Epgp.lua").is_file());
        assert!(!dir.join("Ancien.lua").exists());
        // L'ancienne version est gardée à côté.
        assert_eq!(installed_version(&dir.parent().unwrap().join(".Symposium-ancien")).as_deref(), Some("0.9.0"));
    }

    #[test]
    fn deduit_le_dossier_de_l_addon() {
        let sv = Path::new("C:/WoW/_classic_beta_/WTF/Account/123#2/SavedVariables/Symposium.lua");
        assert_eq!(addon_dir(sv).unwrap(), Path::new("C:/WoW/_classic_beta_/Interface/AddOns/Symposium"));
    }
}
