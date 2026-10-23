//! Versions du jeu installées côte à côte (_classic_beta_, Stress Test…) : chacune a
//! ses dossiers WTF, Interface et Logs. L'application suit celle où l'on vient de
//! jouer, pour que la fiche, l'addon et le journal de combat soient les bons.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// Une version doit avoir servi nettement plus récemment pour qu'on la suive.
const MARGIN: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, PartialEq)]
pub struct Played {
    /// SavedVariables\Symposium.lua de cette version (le fichier peut ne pas exister encore).
    pub saved_variables: PathBuf,
    /// Nom du dossier (_classic_beta_…).
    pub flavor: String,
    /// L'addon Symposium y est installé.
    pub has_addon: bool,
}

/// Nom lisible d'une version du jeu.
pub fn label(flavor: &str) -> String {
    match flavor {
        "_classic_beta_" => "la Bêta de WoW: Forever".into(),
        f if f.contains("ptr") => format!("le serveur de test ({f})"),
        f => format!("la version {f}"),
    }
}

/// Dossier de la version du jeu : …\<version>\WTF\Account\<compte>\SavedVariables\Symposium.lua.
pub fn flavor_dir(saved_variables: &Path) -> Option<PathBuf> {
    Some(saved_variables.parent()?.parent()?.parent()?.parent()?.parent()?.to_path_buf())
}

/// Les versions installées à côté de celle du fichier (elle comprise).
pub fn siblings(saved_variables: &Path) -> Vec<PathBuf> {
    let Some(root) = flavor_dir(saved_variables).and_then(|f| f.parent().map(Path::to_path_buf)) else { return Vec::new() };
    let Ok(entries) = fs::read_dir(&root) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with('_'))
        .map(|e| e.path())
        .filter(|p| p.join("WTF").join("Account").is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Dernière fois que le jeu a tourné dans cette version : journaux du lancement
/// (Logs\*.log), journaux de combat, Config.wtf écrit à la fermeture.
fn last_played(flavor: &Path) -> Option<SystemTime> {
    let logs = fs::read_dir(flavor.join("Logs"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.ends_with(".log") || name.starts_with("WoWCombatLog")
        })
        .filter_map(|e| modified(&e.path()));
    logs.chain(modified(&flavor.join("WTF").join("Config.wtf"))).max()
}

/// Compte à utiliser dans une autre version : le même s'il existe, sinon le plus récent.
fn account(flavor: &Path, same: Option<&str>) -> Option<String> {
    let accounts = flavor.join("WTF").join("Account");
    if let Some(name) = same.filter(|n| accounts.join(n).is_dir()) {
        return Some(name.to_string());
    }
    fs::read_dir(&accounts)
        .ok()?
        .flatten()
        .filter(|e| e.path().join("SavedVariables").is_dir())
        .filter_map(|e| Some((e.file_name().to_string_lossy().to_string(), modified(&e.path())?)))
        .max_by_key(|(_, t)| *t)
        .map(|(name, _)| name)
}

/// La version où l'on a joué depuis, si ce n'est plus celle du fichier suivi.
pub fn played_elsewhere(saved_variables: &Path) -> Option<Played> {
    let current = flavor_dir(saved_variables)?;
    let current_time = last_played(&current).unwrap_or(SystemTime::UNIX_EPOCH);
    let same_account = saved_variables.parent()?.parent()?.file_name()?.to_string_lossy().to_string();

    let (flavor, _) = siblings(saved_variables)
        .into_iter()
        .filter(|f| *f != current)
        .filter_map(|f| Some((f.clone(), last_played(&f)?)))
        .filter(|(_, t)| *t > current_time + MARGIN)
        .max_by_key(|(_, t)| *t)?;
    let account = account(&flavor, Some(&same_account))?;
    Some(Played {
        saved_variables: flavor.join("WTF").join("Account").join(account).join("SavedVariables").join("Symposium.lua"),
        flavor: flavor.file_name()?.to_string_lossy().to_string(),
        has_addon: flavor.join("Interface").join("AddOns").join("Symposium").is_dir(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn touch(path: &Path, ago: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = File::create(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(ago)).unwrap();
    }

    #[test]
    fn suit_la_version_ou_l_on_vient_de_jouer() {
        let root = std::env::temp_dir().join(format!("symposium-flavors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let beta = root.join("_classic_beta_");
        let stress = root.join("_classic_ptr_");
        let sv = beta.join("WTF/Account/123#2/SavedVariables/Symposium.lua");
        touch(&sv, 3 * 86400);
        touch(&beta.join("Logs/Hotfix.log"), 2 * 86400);
        touch(&stress.join("WTF/Account/123#2/SavedVariables/Blizzard.lua"), 3600);
        fs::create_dir_all(stress.join("Interface/AddOns/Symposium")).unwrap();

        // Rien joué dans le Stress Test pour l'instant : on reste sur la Bêta.
        assert_eq!(played_elsewhere(&sv), None);

        touch(&stress.join("Logs/Hotfix.log"), 3600);
        let played = played_elsewhere(&sv).unwrap();
        assert_eq!(played.flavor, "_classic_ptr_");
        assert!(played.has_addon);
        assert_eq!(played.saved_variables, stress.join("WTF/Account/123#2/SavedVariables/Symposium.lua"));
        assert_eq!(siblings(&sv), vec![beta.clone(), stress.clone()]);

        // De retour sur la Bêta : on la retrouve.
        touch(&beta.join("Logs/Hotfix.log"), 0);
        assert_eq!(played_elsewhere(&played.saved_variables).unwrap().flavor, "_classic_beta_");
        let _ = fs::remove_dir_all(&root);
    }
}
