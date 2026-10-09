//! Journal de combat du jeu (/combatlog → Logs/WoWCombatLog-*.txt) : découpé
//! par combat de boss (ENCOUNTER_START … ENCOUNTER_END), résumé (dégâts, soins,
//! morts de chaque joueur du groupe) puis envoyé au site. Le journal lui-même ne
//! quitte pas l'ordinateur.

use crate::{api, settings::SyncReport, sync::Shared};
use chrono::{Local, NaiveDateTime, TimeZone};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Emitter};

/// Derniers coups reçus gardés pour expliquer une mort.
const RECAP: usize = 5;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    pub encounter_id: i64,
    pub name: String,
    pub zone: Option<String>,
    pub difficulty: i64,
    pub group_size: i64,
    /// Secondes depuis 1970.
    pub started_at: i64,
    pub duration: f64,
    pub success: bool,
    pub players: Vec<PlayerStats>,
    pub deaths: Vec<Death>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub name: String,
    pub damage: u64,
    pub healing: u64,
    pub overhealing: u64,
    pub absorbs: u64,
    pub damage_taken: u64,
    pub deaths: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    /// Secondes depuis le début du combat.
    pub at: f64,
    pub source: String,
    pub spell: String,
    pub amount: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Death {
    pub name: String,
    pub at: f64,
    pub hits: Vec<Hit>,
}

struct Fight {
    id: i64,
    name: String,
    difficulty: i64,
    group_size: i64,
    start: f64,
    players: HashMap<String, PlayerStats>,
    recent: HashMap<String, VecDeque<Hit>>,
    deaths: Vec<Death>,
}

#[derive(Default)]
pub struct Parser {
    fight: Option<Fight>,
    zone: Option<String>,
    /// Familier → joueur.
    pets: HashMap<String, String>,
    /// Joueur → nom, pour nommer le maître d'un familier.
    names: HashMap<String, String>,
}

/// Statistiques d'un joueur du combat (créées au premier événement).
fn stats<'a>(players: &'a mut HashMap<String, PlayerStats>, guid: String, name: String) -> &'a mut PlayerStats {
    let entry = players.entry(guid).or_default();
    if entry.name.is_empty() && !name.is_empty() {
        entry.name = name;
    }
    entry
}

/// « Tux-ClassicBetaPvE2- » → « Tux ».
fn short(name: &str) -> String {
    name.split('-').next().unwrap_or(name).to_string()
}

fn is_player(guid: &str) -> bool {
    guid.starts_with("Player-")
}

/// Joueur du groupe ou du raid (pas un inconnu croisé dans la zone).
fn grouped(flags: &str) -> bool {
    u32::from_str_radix(flags.trim_start_matches("0x"), 16).map_or(false, |f| f & 0x7 != 0)
}

/// Champs séparés par des virgules, en respectant les guillemets.
fn split(payload: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let (mut current, mut quoted) = (String::new(), false);
    for c in payload.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// « 10/9/2026 07:53:11.4072 » (heure locale du jeu) → secondes depuis 1970.
fn timestamp(text: &str) -> Option<f64> {
    let text = text.trim();
    let parsed = NaiveDateTime::parse_from_str(text, "%m/%d/%Y %H:%M:%S%.f").ok().or_else(|| {
        // Anciennes versions : sans l'année.
        let year = Local::now().format("%Y");
        let (date, time) = text.split_once(' ')?;
        NaiveDateTime::parse_from_str(&format!("{date}/{year} {time}"), "%m/%d/%Y %H:%M:%S%.f").ok()
    })?;
    let local = Local.from_local_datetime(&parsed).earliest()?;
    Some(local.timestamp_millis() as f64 / 1000.0)
}

fn num(field: Option<&String>) -> u64 {
    field.and_then(|f| f.parse::<f64>().ok()).filter(|v| *v > 0.0).map_or(0, |v| v as u64)
}

impl Parser {
    /// Lit une ligne du journal ; renvoie le combat qu'elle termine, le cas échéant.
    pub fn feed(&mut self, line: &str) -> Option<Encounter> {
        let (time, payload) = line.split_once("  ")?;
        let at = timestamp(time)?;
        let mut f = split(payload.trim_end());
        let event = f[0].clone();

        match event.as_str() {
            "ZONE_CHANGE" => self.zone = f.get(2).cloned(),
            "ENCOUNTER_START" => {
                self.fight = Some(Fight {
                    id: f.get(1)?.parse().ok()?,
                    name: f.get(2)?.clone(),
                    difficulty: f.get(3).and_then(|v| v.parse().ok()).unwrap_or(0),
                    group_size: f.get(4).and_then(|v| v.parse().ok()).unwrap_or(0),
                    start: at,
                    players: HashMap::new(),
                    recent: HashMap::new(),
                    deaths: Vec::new(),
                });
            }
            "ENCOUNTER_END" => {
                let id: i64 = f.get(1)?.parse().ok()?;
                if self.fight.as_ref().map(|fight| fight.id) != Some(id) {
                    return None;
                }
                let fight = self.fight.take()?;
                let success = f.get(5).map(|v| v == "1").unwrap_or(false);
                let mut players: Vec<PlayerStats> = fight
                    .players
                    .into_iter()
                    .map(|(guid, mut p)| {
                        if p.name.is_empty() {
                            p.name = self.names.get(&guid).cloned().unwrap_or_else(|| "Inconnu".into());
                        }
                        p
                    })
                    .collect();
                players.sort_by(|a, b| b.damage.cmp(&a.damage).then(b.healing.cmp(&a.healing)));
                return Some(Encounter {
                    encounter_id: fight.id,
                    name: fight.name,
                    zone: self.zone.clone(),
                    difficulty: fight.difficulty,
                    group_size: fight.group_size,
                    started_at: fight.start as i64,
                    duration: ((at - fight.start) * 10.0).round() / 10.0,
                    success,
                    players,
                    deaths: fight.deaths,
                });
            }
            _ => {}
        }

        if f.len() < 9 {
            return None;
        }
        // Dégâts des sorts : un dernier champ indique la cible unique ou la zone.
        if matches!(f.last().map(String::as_str), Some("ST") | Some("AOE")) {
            f.pop();
        }
        let (src, src_name, src_flags, dst, dst_name, dst_flags) = (&f[1], &f[2], &f[3], &f[5], &f[6], &f[7]);
        for (guid, name) in [(src, src_name), (dst, dst_name)] {
            if is_player(guid) && name != "nil" {
                self.names.entry(guid.clone()).or_insert_with(|| short(name));
            }
        }
        let spell_event = event.starts_with("SPELL_") || event.starts_with("RANGE_") || event.starts_with("DAMAGE_");

        // Familiers : leur maître est indiqué dans les informations avancées, ou à l'invocation.
        let advanced = if spell_event { 12 } else { 9 };
        if let (Some(info), Some(owner)) = (f.get(advanced), f.get(advanced + 1)) {
            if info == src && is_player(owner) && !is_player(src) {
                self.pets.insert(src.clone(), owner.clone());
            }
        }
        if event == "SPELL_SUMMON" && is_player(src) {
            self.pets.insert(dst.clone(), src.clone());
        }

        let pets = &self.pets;
        let fight = self.fight.as_mut()?;
        let elapsed = ((at - fight.start) * 10.0).round() / 10.0;
        // Joueur à créditer : lui-même, ou le maître du familier.
        let credited = |guid: &str, name: &str, flags: &str| -> Option<(String, String)> {
            if is_player(guid) {
                return grouped(flags).then(|| (guid.to_string(), short(name)));
            }
            // Familier d'un membre du groupe (le sien porte les mêmes drapeaux de groupe).
            pets.get(guid).filter(|_| grouped(flags)).map(|owner| (owner.clone(), String::new()))
        };

        match event.as_str() {
            "SWING_DAMAGE" | "SPELL_DAMAGE" | "SPELL_PERIODIC_DAMAGE" | "RANGE_DAMAGE" | "DAMAGE_SHIELD" | "DAMAGE_SPLIT" => {
                let amount = num(f.get(f.len().checked_sub(10)?));
                if !is_player(dst) {
                    if let Some((guid, name)) = credited(src, src_name, src_flags) {
                        stats(&mut fight.players, guid, name).damage += amount;
                    }
                } else if grouped(dst_flags) {
                    stats(&mut fight.players, dst.clone(), short(dst_name)).damage_taken += amount;
                    let spell = if event == "SWING_DAMAGE" { "Mêlée".to_string() } else { f.get(10).cloned().unwrap_or_default() };
                    let recent = fight.recent.entry(dst.clone()).or_default();
                    recent.push_back(Hit { at: elapsed, source: short(src_name), spell, amount });
                    if recent.len() > RECAP {
                        recent.pop_front();
                    }
                }
            }
            "SPELL_HEAL" | "SPELL_PERIODIC_HEAL" => {
                let amount = num(f.get(f.len().checked_sub(5)?));
                let over = num(f.get(f.len().checked_sub(3)?));
                if let Some((guid, name)) = credited(src, src_name, src_flags) {
                    let s = stats(&mut fight.players, guid, name);
                    s.healing += amount.saturating_sub(over);
                    s.overhealing += over;
                }
            }
            "SPELL_ABSORBED" => {
                // … lanceur du bouclier (GUID, nom, drapeaux ×2), sort (×3), montant, montant de base, critique.
                let n = f.len();
                if n >= 18 {
                    let (guid, name, flags) = (&f[n - 10], &f[n - 9], &f[n - 8]);
                    if let Some((guid, name)) = credited(guid, name, flags) {
                        stats(&mut fight.players, guid, name).absorbs += num(f.get(n - 3));
                    }
                }
            }
            "UNIT_DIED" if is_player(dst) && grouped(dst_flags) => {
                stats(&mut fight.players, dst.clone(), short(dst_name)).deaths += 1;
                let hits = fight.recent.remove(dst).map(Vec::from).unwrap_or_default();
                fight.deaths.push(Death { name: short(dst_name), at: elapsed, hits });
            }
            _ => {}
        }
        None
    }
}

/// Dossier Logs du jeu, à côté de WTF (déduit du fichier de l'addon).
pub fn logs_dir(saved_variables: &Path) -> Option<PathBuf> {
    // SavedVariables → <compte> → Account → WTF → <version du jeu>
    let flavor = saved_variables.parent()?.parent()?.parent()?.parent()?.parent()?;
    Some(flavor.join("Logs"))
}

/// Journal le plus récent, s'il a été écrit dans les 12 dernières heures.
fn newest(dir: &Path) -> Option<PathBuf> {
    let recent = SystemTime::now() - Duration::from_secs(12 * 3600);
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("WoWCombatLog") && name.ends_with(".txt")
        })
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .filter(|(_, modified)| *modified > recent)
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path)
}

struct Reader {
    path: PathBuf,
    offset: u64,
    rest: String,
    parser: Parser,
}

impl Reader {
    /// Lignes ajoutées depuis la dernière lecture ; renvoie les combats terminés.
    fn read(&mut self) -> Vec<Encounter> {
        let Ok(mut file) = fs::File::open(&self.path) else { return Vec::new() };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            // Fichier recommencé : on repart du début.
            self.offset = 0;
            self.rest.clear();
            self.parser = Parser::default();
        }
        let mut bytes = Vec::new();
        if file.seek(SeekFrom::Start(self.offset)).is_err() || file.read_to_end(&mut bytes).is_err() {
            return Vec::new();
        }
        self.offset += bytes.len() as u64;
        let mut text = std::mem::take(&mut self.rest);
        text.push_str(&String::from_utf8_lossy(&bytes));
        // La dernière ligne peut être en cours d'écriture : gardée pour la prochaine fois.
        let complete = text.rfind('\n').map_or(0, |i| i + 1);
        self.rest = text[complete..].to_string();
        text[..complete].lines().filter_map(|line| self.parser.feed(line)).collect()
    }
}

/// Surveille le journal de combat toutes les 5 secondes et envoie chaque combat de boss terminé.
pub fn watch(app: AppHandle, state: Shared, config_dir: PathBuf) {
    thread::spawn(move || {
        let mut reader: Option<Reader> = None;
        let mut pending: Vec<Encounter> = Vec::new();
        let mut marker: Option<(String, bool, bool)> = None;
        let mut tick: u32 = 0;
        loop {
            thread::sleep(Duration::from_secs(5));
            tick = tick.wrapping_add(1);
            let settings = state.lock().unwrap().clone();
            // Addon témoin : installé quand l'application est configurée ; il porte la case
            // « Envoyer mes combats » (journal automatique) et le classement EPGP.
            let wanted = (settings.saved_variables.clone(), settings.ready(), settings.upload_combat_logs);
            if marker.as_ref() != Some(&wanted) && !settings.saved_variables.is_empty() {
                if crate::addon::sync_marker(Path::new(&settings.saved_variables), wanted.1, wanted.2).is_ok() {
                    marker = Some(wanted);
                    tick = 0;
                }
            }
            // Classement EPGP : au démarrage puis toutes les 10 minutes (ignoré pour un non-membre).
            if settings.ready() && tick % 120 == 1 {
                if let Ok(code) = api::standings(&settings.site_url, &settings.token) {
                    let _ = crate::addon::write_standings(Path::new(&settings.saved_variables), &code);
                }
            }
            if !settings.ready() || !settings.upload_combat_logs {
                continue;
            }
            let Some(path) = logs_dir(Path::new(&settings.saved_variables)).and_then(|dir| newest(&dir)) else { continue };
            if reader.as_ref().map_or(true, |r| r.path != path) {
                reader = Some(Reader { path, offset: 0, rest: String::new(), parser: Parser::default() });
            }
            pending.extend(reader.as_mut().unwrap().read());

            // Envoi dans l'ordre ; ce qui échoue (site injoignable) est retenté au tour suivant.
            while let Some(encounter) = pending.first().cloned() {
                let report = match api::upload_encounter(&settings.site_url, &settings.token, &encounter) {
                    Ok(done) => SyncReport {
                        at: chrono::Utc::now().timestamp(),
                        ok: true,
                        message: format!(
                            "Combat envoyé : {} ({}, {} s){}.",
                            encounter.name,
                            if encounter.success { "tué" } else { "essai" },
                            encounter.duration.round(),
                            if done.created { "" } else { ", déjà reçu d’un autre joueur" }
                        ),
                        character: None,
                        url: Some(done.url),
                        export_at: Some(encounter.started_at),
                    },
                    Err(message) => {
                        let retry = message.starts_with("Site injoignable");
                        let report = SyncReport { at: chrono::Utc::now().timestamp(), ok: false, message, character: None, url: None, export_at: Some(encounter.started_at) };
                        if retry {
                            save(&app, &state, &config_dir, report);
                            break;
                        }
                        report
                    }
                };
                pending.remove(0);
                save(&app, &state, &config_dir, report);
            }
            pending.truncate(50);
        }
    });
}

fn save(app: &AppHandle, state: &Shared, config_dir: &Path, report: SyncReport) {
    let mut s = state.lock().unwrap();
    s.last_combat = Some(report.clone());
    let _ = s.save(config_dir);
    let _ = app.emit("combat", &report);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lignes du journal de WoW: Forever 1.60.1 (noms changés), plus un combat de boss.
    const LOG: &str = r#"10/9/2026 07:53:11.4072  COMBAT_LOG_VERSION,22,ADVANCED_LOG_ENABLED,0,BUILD_VERSION,1.60.1,PROJECT_ID,18
10/9/2026 07:53:11.4072  ZONE_CHANGE,0,"Les Paluns (Wetlands)",0
10/9/2026 07:53:12.0000  ENCOUNTER_START,1144,"Grub",1,5,48
10/9/2026 07:53:12.5402  SWING_DAMAGE,Pet-0-6782-1-3037-165189-01003348FC,"Cuddles",0x1112,0x80000000,Creature-0-4615-0-6503-1417-0000487C74,"Jeune crocilisque des Paluns",0xa28,0x80000000,Pet-0-6782-1-3037-165189-01003348FC,Player-4618-01547D62,774,869,96,0,1269,0,0,0,2,18,100,0,-3144.51,-1573.13,1437,1.0124,10,32,45,-1,1,0,0,0,nil,nil,nil
10/9/2026 07:53:13.0652  SWING_DAMAGE_LANDED,Creature-0-4615-0-6503-1417-0000487C74,"Jeune crocilisque des Paluns",0xa28,0x80000000,Pet-0-6782-1-3037-165189-01003348FC,"Cuddles",0x1112,0x80000000,Pet-0-6782-1-3037-165189-01003348FC,Player-4618-01547D62,750,869,96,0,1269,0,0,0,2,18,100,0,-3144.51,-1573.13,1437,1.0124,10,24,39,-1,1,0,0,0,nil,nil,nil
10/9/2026 07:53:26.3292  SPELL_DAMAGE,Player-4620-008EF1C4,"Lame-ClassicBetaPvE2-",0x511,0x80000000,Creature-0-4615-0-6503-1417-0000487521,"Jeune crocilisque des Paluns",0x10a48,0x80000000,1759,"Attaque pernicieuse",0x1,Creature-0-4615-0-6503-1417-0000487521,0000000000000000,599,691,74,0,897,0,0,0,1,0,0,0,-3177.48,-1756.39,1437,6.0284,21,48,63,-1,1,0,0,0,nil,nil,nil,ST
10/9/2026 07:53:26.5000  SPELL_DAMAGE,Player-4620-00000001,"Etranger-ClassicBetaPvE2-",0x548,0x80000000,Creature-0-4615-0-6503-1417-0000487521,"Jeune crocilisque des Paluns",0x10a48,0x80000000,1759,"Attaque pernicieuse",0x1,Creature-0-4615-0-6503-1417-0000487521,0000000000000000,599,691,74,0,897,0,0,0,1,0,0,0,-3177.48,-1756.39,1437,6.0284,21,999,999,-1,1,0,0,0,nil,nil,nil,ST
10/9/2026 07:53:30.0000  SWING_DAMAGE,Creature-0-4615-0-6503-1040-0000487CC3,"Rampant des tourbières",0xa48,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,Creature-0-4615-0-6503-1040-0000487CC3,0000000000000000,768,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,40,45,-1,1,0,0,0,nil,nil,nil
10/9/2026 07:53:47.1982  SPELL_ABSORBED,Creature-0-4615-0-6503-1040-0000487CC3,"Rampant des tourbières",0xa48,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,3747,"Mot de pouvoir : Bouclier",0x2,31,35,nil
10/9/2026 07:53:48.0000  SPELL_HEAL,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,2050,"Soins inférieurs",0x2,Player-4620-01548E6A,0000000000000000,800,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,120,120,20,0,nil
10/9/2026 07:53:50.0000  SPELL_DAMAGE,Creature-0-4615-0-6503-1040-0000487CC3,"Rampant des tourbières",0xa48,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,5678,"Morsure",0x1,Creature-0-4615-0-6503-1040-0000487CC3,0000000000000000,768,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,900,900,37,1,0,0,0,nil,nil,nil,ST
10/9/2026 07:53:50.1000  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,0
10/9/2026 07:54:12.0000  ENCOUNTER_END,1144,"Grub",1,5,1,60000"#;

    #[test]
    fn resume_un_combat_de_boss() {
        let mut parser = Parser::default();
        let encounters: Vec<Encounter> = LOG.lines().filter_map(|l| parser.feed(l)).collect();
        assert_eq!(encounters.len(), 1);
        let e = &encounters[0];
        assert_eq!((e.encounter_id, e.name.as_str(), e.success, e.duration, e.group_size), (1144, "Grub", true, 60.0, 5));
        assert_eq!(e.zone.as_deref(), Some("Les Paluns (Wetlands)"));

        let by = |name: &str| e.players.iter().find(|p| p.name == name).cloned().unwrap_or_default();
        // Familier compté à son maître (nom inconnu tant qu'il n'a rien fait lui-même) ; coup « LANDED » pas compté deux fois.
        assert_eq!(e.players.iter().find(|p| p.damage == 32).map(|p| p.name.as_str()), Some("Inconnu"));
        assert_eq!(by("Lame").damage, 48);
        // Un joueur hors du groupe n'est pas compté.
        assert!(e.players.iter().all(|p| p.name != "Etranger"));

        let healer = by("Soin");
        assert_eq!((healer.healing, healer.overhealing, healer.absorbs), (100, 20, 31));
        assert_eq!((healer.damage_taken, healer.deaths), (940, 1));
        assert_eq!(e.deaths.len(), 1);
        assert_eq!(e.deaths[0].hits.last().map(|h| (h.spell.as_str(), h.amount)), Some(("Morsure", 900)));
        assert_eq!(e.deaths[0].hits[0].spell, "Mêlée");
    }

    #[test]
    fn ignore_ce_qui_est_hors_combat_de_boss() {
        let mut parser = Parser::default();
        let outside = LOG.lines().filter(|l| !l.contains("ENCOUNTER_"));
        assert!(outside.filter_map(|l| parser.feed(l)).next().is_none());
    }

    /// Un vrai journal (COMBATLOG=chemin) : aucune ligne ne doit faire échouer la lecture.
    #[test]
    fn lit_un_vrai_journal() {
        let Ok(path) = std::env::var("COMBATLOG") else { return };
        let text = std::fs::read_to_string(path).unwrap();
        let mut parser = Parser::default();
        let mut wrapped = String::from("10/9/2026 07:53:11.0000  ENCOUNTER_START,1,\"Test\",1,5,0\n");
        wrapped.push_str(&text);
        wrapped.push_str("10/9/2026 07:59:00.0000  ENCOUNTER_END,1,\"Test\",1,5,1,0\n");
        let found: Vec<Encounter> = wrapped.lines().filter_map(|l| parser.feed(l)).collect();
        println!("{}", serde_json::to_string_pretty(&found).unwrap());
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn lit_l_heure_locale() {
        let at = timestamp("10/9/2026 07:53:11.4072").unwrap();
        let back = Local.timestamp_opt(at as i64, 0).unwrap();
        assert_eq!(back.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-10-09 07:53:11");
    }
}
