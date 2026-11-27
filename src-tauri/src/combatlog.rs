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
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

/// Derniers coups reçus gardés pour expliquer une mort.
const RECAP: usize = 5;
/// Secondes gardées avant une mort pour la raconter : coups, soins reçus, effets ennemis.
const RECAP_SECONDS: f64 = 15.0;
/// Lignes gardées au plus dans ce récit.
const RECAP_MAX: usize = 60;
/// Points de vie relevés il y a plus longtemps que ça : trop vieux pour être montrés.
const HEALTH_FRESH: f64 = 3.0;
/// Fenêtre pour compter les joueurs touchés par le même sort (capacité de zone).
const SPREAD_SECONDS: f64 = 1.5;
/// Secondes sans coup échangé avec un monstre : le pack de trash est terminé.
const TRASH_IDLE: f64 = 8.0;
/// Largeur des tranches de la courbe des dégâts et des soins, en secondes.
const BUCKET: f64 = 5.0;
/// Sorts lancés gardés au plus par combat (un raid de 40 sur dix minutes en lance moins).
const MAX_CASTS: usize = 15_000;

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
    pub detail: Detail,
    /// Pack de monstres hors combat de boss, dans une instance (1.4.0).
    pub trash: bool,
    /// Les monstres du combat, par nom : combien, tués, dégâts reçus et infligés (1.4.0).
    pub enemies: Vec<Enemy>,
}

/// Monstres d'un même nom pendant un combat.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Enemy {
    pub name: String,
    pub count: u32,
    pub killed: u32,
    /// Dégâts que le groupe leur a infligés.
    pub damage_taken: u64,
    /// Dégâts qu'ils ont infligés au groupe.
    pub damage_done: u64,
}

/// Un monstre pendant le combat, en cours de lecture.
#[derive(Default)]
struct EnemyTrack {
    name: String,
    taken: u64,
    done: u64,
    died: bool,
}

/// Le détail du combat : sorts de chaque joueur, courbe, et sorts lancés dans l'ordre.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    /// Largeur des tranches de la courbe, en secondes.
    pub bucket: f64,
    pub players: Vec<PlayerDetail>,
    pub casts: Vec<Cast>,
    /// Dégâts subis par le groupe, par sort (0.12.1).
    pub taken: Vec<Taken>,
}

/// Ce qui a frappé le groupe : un sort (ou la mêlée) et qui l'a lancé.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Taken {
    /// 0 pour la mêlée.
    pub id: i64,
    pub name: String,
    pub source: String,
    pub amount: u64,
    pub hits: u32,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerDetail {
    pub name: String,
    pub spells: Vec<SpellStats>,
    /// Dégâts, soins utiles (absorptions comprises) et dégâts subis, par tranche de BUCKET secondes.
    pub damage: Vec<u64>,
    pub healing: Vec<u64>,
    pub taken: Vec<u64>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpellStats {
    /// 0 pour la mêlée.
    pub id: i64,
    pub name: String,
    pub heal: bool,
    /// Lancé par le familier du joueur.
    pub pet: bool,
    /// Dégâts, ou soins utiles.
    pub amount: u64,
    pub overhealing: u64,
    pub hits: u32,
    pub crits: u32,
}

/// Sort lancé par un joueur du groupe, ou par le boss et ses adds (début d'incantation compris).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cast {
    /// Secondes depuis le début du combat.
    pub at: f64,
    pub source: String,
    pub spell_id: i64,
    pub spell: String,
    pub target: String,
    pub hostile: bool,
    /// Début d'incantation (le sort part ensuite, ou est interrompu).
    pub start: bool,
    /// École de magie du jeu (1 physique, 2 sacré, 4 feu, 8 nature, 16 givre, 32 ombre, 64 arcanes ; combinées), 0.12.2.
    pub school: u32,
}

/// Ce qu'un joueur a fait pendant le combat, en cours de lecture.
#[derive(Default)]
struct Tally {
    spells: HashMap<(i64, String, bool, bool), SpellStats>,
    damage: Vec<u64>,
    healing: Vec<u64>,
    taken: Vec<u64>,
}

/// Ajoute un montant à la tranche du moment.
fn slot(series: &mut Vec<u64>, elapsed: f64, amount: u64) {
    let i = (elapsed.max(0.0) / BUCKET) as usize;
    if series.len() <= i {
        series.resize(i + 1, 0);
    }
    series[i] += amount;
}

impl Tally {
    fn add(&mut self, elapsed: f64, (id, name): (i64, String), heal: bool, pet: bool, amount: u64, over: u64, crit: bool) {
        let spell = self.spells.entry((id, name.clone(), heal, pet)).or_insert_with(|| SpellStats { id, name, heal, pet, ..Default::default() });
        spell.amount += amount;
        spell.overhealing += over;
        spell.hits += 1;
        spell.crits += crit as u32;
        slot(if heal { &mut self.healing } else { &mut self.damage }, elapsed, amount);
    }
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
    /// Les dernières secondes du joueur, dans l'ordre : coups, soins reçus, effets ennemis.
    pub recap: Vec<RecapEvent>,
    /// Points de vie maximum, quand le journal avancé les a donnés.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<u64>,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

/// Une ligne du récit d'une mort.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecapEvent {
    /// Secondes depuis le début du combat.
    pub at: f64,
    /// « damage », « heal », « debuff » (posé par un ennemi) ou « debuffEnd » (retiré).
    pub kind: String,
    pub source: String,
    pub spell: String,
    pub spell_id: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub amount: u64,
    /// Dégâts au-delà des points de vie restants (le coup fatal).
    #[serde(skip_serializing_if = "is_zero")]
    pub overkill: u64,
    /// École du sort (1 physique, 2 sacré, 4 feu, 8 nature, 16 givre, 32 ombre, 64 arcane).
    #[serde(skip_serializing_if = "is_zero")]
    pub school: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub resisted: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub blocked: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub absorbed: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub overheal: u64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub crit: bool,
    /// Points de vie du joueur à ce moment (en %), d'après sa dernière action relevée.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp: Option<u8>,
    /// Joueurs du groupe touchés par ce même sort au même moment (capacité de zone).
    #[serde(skip_serializing_if = "is_zero")]
    pub aoe: u64,
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
    tallies: HashMap<String, Tally>,
    casts: Vec<Cast>,
    taken: HashMap<(i64, String), Taken>,
    /// Récit en cours de chaque joueur (fenêtre glissante).
    recap: HashMap<String, VecDeque<RecapEvent>>,
    /// Derniers points de vie relevés de chaque joueur : actuels, maximum, moment.
    health: HashMap<String, (u64, u64, f64)>,
    /// Sorts ennemis reçus par le groupe ces dernières secondes : moment, sort, joueur touché.
    spread: VecDeque<(f64, i64, String)>,
    /// Monstres touchés ou qui ont touché le groupe.
    enemies: HashMap<String, EnemyTrack>,
    /// Dernier coup échangé avec un monstre (heure du journal).
    last: f64,
    trash: bool,
}

impl Fight {
    fn new(id: i64, name: String, difficulty: i64, group_size: i64, start: f64, trash: bool) -> Fight {
        Fight {
            id,
            name,
            difficulty,
            group_size,
            start,
            players: HashMap::new(),
            recent: HashMap::new(),
            deaths: Vec::new(),
            tallies: HashMap::new(),
            casts: Vec::new(),
            taken: HashMap::new(),
            recap: HashMap::new(),
            health: HashMap::new(),
            spread: VecDeque::new(),
            enemies: HashMap::new(),
            last: start,
            trash,
        }
    }

    fn hp(&self, guid: &str, at: f64) -> Option<u8> {
        self.health
            .get(guid)
            .filter(|(_, max, seen)| *max > 0 && at - seen <= HEALTH_FRESH)
            .map(|(cur, max, _)| ((*cur as f64 / *max as f64) * 100.0).round().min(100.0) as u8)
    }

    fn remember(&mut self, guid: &str, event: RecapEvent) {
        let line = self.recap.entry(guid.to_string()).or_default();
        let at = event.at;
        line.push_back(event);
        while line.front().is_some_and(|e| e.at < at - RECAP_SECONDS) || line.len() > RECAP_MAX {
            line.pop_front();
        }
    }
}

/// Nombre du journal, -1 (« rien ») compté zéro.
fn amount_at(f: &[String], back: usize) -> u64 {
    f.len().checked_sub(back).and_then(|i| f.get(i)).and_then(|v| v.parse::<i64>().ok()).filter(|v| *v > 0).map_or(0, |v| v as u64)
}

#[derive(Default)]
pub struct Parser {
    fight: Option<Fight>,
    /// Pack de trash en cours (dans une instance, hors combat de boss).
    trash: Option<Fight>,
    /// Dans une instance (donjon, raid) : le trash y est relevé.
    instance: bool,
    /// Combats terminés pas encore rendus (un pack de trash se termine avec la ligne suivante).
    ready: VecDeque<Encounter>,
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

/// Personnage hostile (le boss, ses adds).
/// Réaction amicale (drapeau 0x10) : un allié, jamais un monstre.
fn friendly(flags: &str) -> bool {
    u32::from_str_radix(flags.trim_start_matches("0x"), 16).map_or(true, |f| f & 0x10 != 0)
}

fn hostile(flags: &str) -> bool {
    u32::from_str_radix(flags.trim_start_matches("0x"), 16).map_or(false, |f| f & 0x40 != 0)
}

/// Critique : « 1 » dans le journal, « nil » sinon.
fn crit(field: Option<&String>) -> bool {
    field.is_some_and(|v| v == "1")
}

fn spell_of(f: &[String]) -> (i64, String) {
    (f.get(9).and_then(|v| v.parse().ok()).unwrap_or(0), f.get(10).cloned().unwrap_or_default())
}

fn num(field: Option<&String>) -> u64 {
    field.and_then(|f| f.parse::<f64>().ok()).filter(|v| *v > 0.0).map_or(0, |v| v as u64)
}

impl Parser {
    /// Lit une ligne du journal ; renvoie un combat terminé (boss ou pack de trash), le cas échéant.
    pub fn feed(&mut self, line: &str) -> Option<Encounter> {
        if let Some(done) = self.step(line) {
            self.ready.push_back(done);
        }
        self.ready.pop_front()
    }

    /// Fin du journal (ou plus rien d'écrit) : le pack de trash en cours est terminé.
    pub fn flush(&mut self) -> Vec<Encounter> {
        self.close_trash();
        self.ready.drain(..).collect()
    }

    fn close_trash(&mut self) {
        if let Some(fight) = self.trash.take() {
            let end = fight.last;
            if let Some(done) = self.close(fight, end, None) {
                self.ready.push_back(done);
            }
        }
    }

    fn step(&mut self, line: &str) -> Option<Encounter> {
        let (time, payload) = line.split_once("  ")?;
        let at = timestamp(time)?;
        let f = split(payload.trim_end());
        let event = f[0].clone();
        if self.trash.as_ref().is_some_and(|t| at - t.last > TRASH_IDLE) {
            self.close_trash();
        }

        match event.as_str() {
            "ZONE_CHANGE" => {
                self.close_trash();
                self.zone = f.get(2).cloned();
                // Numéro d'instance : 0 hors des donjons et des raids.
                self.instance = f.get(1).is_some_and(|id| id != "0");
            }
            "ENCOUNTER_START" => {
                self.close_trash();
                self.fight = Some(Fight::new(
                    f.get(1)?.parse().ok()?,
                    f.get(2)?.clone(),
                    f.get(3).and_then(|v| v.parse().ok()).unwrap_or(0),
                    f.get(4).and_then(|v| v.parse().ok()).unwrap_or(0),
                    at,
                    false,
                ));
            }
            "ENCOUNTER_END" => {
                let id: i64 = f.get(1)?.parse().ok()?;
                if self.fight.as_ref().map(|fight| fight.id) != Some(id) {
                    return None;
                }
                let fight = self.fight.take()?;
                let success = f.get(5).map(|v| v == "1").unwrap_or(false);
                return self.close(fight, at, Some(success));
            }
            _ => {}
        }
        self.track(at, f, &event)
    }

    /// Résumé d'un combat terminé. `success` : issue du boss ; None pour un pack de trash (réussi si
    /// tous ses monstres sont morts, gardé seulement si l'un est mort ou un joueur).
    fn close(&self, fight: Fight, end: f64, success: Option<bool>) -> Option<Encounter> {
        let mut enemies: Vec<Enemy> = Vec::new();
        for track in fight.enemies.values() {
            let name = if track.name.is_empty() { "Inconnu".to_string() } else { track.name.clone() };
            let i = match enemies.iter().position(|e| e.name == name) {
                Some(i) => i,
                None => {
                    enemies.push(Enemy { name, ..Default::default() });
                    enemies.len() - 1
                }
            };
            let e = &mut enemies[i];
            e.count += 1;
            e.killed += track.died as u32;
            e.damage_taken += track.taken;
            e.damage_done += track.done;
        }
        enemies.sort_by(|a, b| b.damage_taken.cmp(&a.damage_taken).then(a.name.cmp(&b.name)));
        let (name, success) = match success {
            Some(success) => (fight.name.clone(), success),
            None => {
                let killed = enemies.iter().map(|e| e.killed).sum::<u32>();
                if killed == 0 && fight.deaths.is_empty() {
                    return None;
                }
                let top = enemies.first()?;
                let mut name = if top.count > 1 { format!("{} ×{}", top.name, top.count) } else { top.name.clone() };
                if enemies.len() > 1 {
                    name.push_str(&format!(" + {}", enemies[1..].iter().map(|e| e.count).sum::<u32>()));
                }
                (name, enemies.iter().all(|e| e.killed == e.count))
            }
        };
        let name_of = |guid: &String, known: &str| {
            if known.is_empty() { self.names.get(guid).cloned().unwrap_or_else(|| "Inconnu".into()) } else { known.to_string() }
        };
        let mut details: Vec<PlayerDetail> = fight
            .tallies
            .into_iter()
            .map(|(guid, t)| {
                let mut spells: Vec<SpellStats> = t.spells.into_values().collect();
                spells.sort_by(|a, b| b.amount.cmp(&a.amount));
                let known = fight.players.get(&guid).map(|p| p.name.clone()).unwrap_or_default();
                PlayerDetail { name: name_of(&guid, &known), spells, damage: t.damage, healing: t.healing, taken: t.taken }
            })
            .collect();
        details.sort_by_key(|d| std::cmp::Reverse(d.damage.iter().sum::<u64>()));
        let mut taken: Vec<Taken> = fight.taken.into_values().collect();
        taken.sort_by(|a, b| b.amount.cmp(&a.amount));
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
        let group_size = if fight.trash { players.len() as i64 } else { fight.group_size };
        Some(Encounter {
            encounter_id: fight.id,
            name,
            zone: self.zone.clone(),
            difficulty: fight.difficulty,
            group_size,
            started_at: fight.start as i64,
            duration: ((end - fight.start) * 10.0).round() / 10.0,
            success,
            players,
            deaths: fight.deaths,
            detail: Detail { bucket: BUCKET, players: details, casts: fight.casts, taken },
            trash: fight.trash,
            enemies,
        })
    }

    /// Une ligne de combat : comptée au boss en cours, sinon au pack de trash (qu'elle ouvre au besoin).
    fn track(&mut self, at: f64, mut f: Vec<String>, event: &str) -> Option<Encounter> {
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
        // Coup échangé entre le groupe (ou un familier) et un monstre : ouvre ou prolonge le pack de trash.
        let damage = matches!(event, "SWING_DAMAGE" | "SPELL_DAMAGE" | "SPELL_PERIODIC_DAMAGE" | "RANGE_DAMAGE" | "DAMAGE_SHIELD" | "DAMAGE_SPLIT");
        let ours = |guid: &str, flags: &str| (is_player(guid) || pets.contains_key(guid)) && grouped(flags);
        // Monstre : ni joueur, ni familier, ni amical (les neutres attaqués comptent).
        let enemy = |guid: &str, flags: &str| !is_player(guid) && !pets.contains_key(guid) && !friendly(flags);
        let engaged = damage && ((ours(src, src_flags) && enemy(dst, dst_flags)) || (enemy(src, src_flags) && ours(dst, dst_flags)));
        if self.fight.is_none() && self.trash.is_none() && engaged && self.instance {
            self.trash = Some(Fight::new(0, String::new(), 0, 0, at, true));
        }
        let fight = match self.fight.as_mut() {
            Some(fight) => fight,
            None => self.trash.as_mut()?,
        };
        if engaged {
            fight.last = at;
        }
        let elapsed = ((at - fight.start) * 10.0).round() / 10.0;
        // Journal avancé : points de vie de celui qui agit (actuels, maximum).
        if let (Some(info), Some(cur), Some(max)) = (f.get(advanced), f.get(advanced + 2), f.get(advanced + 3)) {
            if info == src && is_player(src) {
                if let (Ok(cur), Ok(max)) = (cur.parse::<u64>(), max.parse::<u64>()) {
                    if max > 0 {
                        fight.health.insert(src.clone(), (cur, max, elapsed));
                    }
                }
            }
        }
        // Joueur à créditer : lui-même, ou le maître du familier.
        let credited = |guid: &str, name: &str, flags: &str| -> Option<(String, String)> {
            if is_player(guid) {
                return grouped(flags).then(|| (guid.to_string(), short(name)));
            }
            // Familier d'un membre du groupe (le sien porte les mêmes drapeaux de groupe).
            pets.get(guid).filter(|_| grouped(flags)).map(|owner| (owner.clone(), String::new()))
        };

        match event {
            "SWING_DAMAGE" | "SPELL_DAMAGE" | "SPELL_PERIODIC_DAMAGE" | "RANGE_DAMAGE" | "DAMAGE_SHIELD" | "DAMAGE_SPLIT" => {
                let amount = num(f.get(f.len().checked_sub(10)?));
                if !is_player(dst) {
                    if credited(src, src_name, src_flags).is_some() && enemy(dst, dst_flags) {
                        let track = fight.enemies.entry(dst.clone()).or_insert_with(|| EnemyTrack { name: short(dst_name), ..Default::default() });
                        track.taken += amount;
                    }
                    if let Some((guid, name)) = credited(src, src_name, src_flags) {
                        stats(&mut fight.players, guid.clone(), name).damage += amount;
                        let spell = if event == "SWING_DAMAGE" { (0, "Mêlée".to_string()) } else { spell_of(&f) };
                        let critical = crit(f.get(f.len() - 3));
                        fight.tallies.entry(guid).or_default().add(elapsed, spell, false, !is_player(src), amount, 0, critical);
                    }
                } else if grouped(dst_flags) {
                    if enemy(src, src_flags) {
                        let track = fight.enemies.entry(src.clone()).or_insert_with(|| EnemyTrack { name: short(src_name), ..Default::default() });
                        track.done += amount;
                    }
                    stats(&mut fight.players, dst.clone(), short(dst_name)).damage_taken += amount;
                    let (id, spell) = if event == "SWING_DAMAGE" { (0, "Mêlée".to_string()) } else { spell_of(&f) };
                    slot(&mut fight.tallies.entry(dst.clone()).or_default().taken, elapsed, amount);
                    let by = fight.taken.entry((id, spell.clone())).or_insert_with(|| Taken { id, name: spell.clone(), source: short(src_name), ..Default::default() });
                    by.amount += amount;
                    by.hits += 1;
                    let recent = fight.recent.entry(dst.clone()).or_default();
                    recent.push_back(Hit { at: elapsed, source: short(src_name), spell: spell.clone(), amount });
                    if recent.len() > RECAP {
                        recent.pop_front();
                    }
                    // Fin des dégâts : montant, de base, surplus, école, résisté, bloqué, absorbé, critique…
                    let hp = fight.hp(dst, elapsed);
                    fight.remember(dst, RecapEvent {
                        at: elapsed,
                        kind: "damage".into(),
                        source: short(src_name),
                        spell,
                        spell_id: id,
                        amount,
                        overkill: amount_at(&f, 8),
                        school: amount_at(&f, 7),
                        resisted: amount_at(&f, 6),
                        blocked: amount_at(&f, 5),
                        absorbed: amount_at(&f, 4),
                        crit: crit(f.get(f.len() - 3)),
                        hp,
                        ..Default::default()
                    });
                    if id != 0 && !is_player(src) {
                        fight.spread.push_back((elapsed, id, dst.clone()));
                        while fight.spread.front().is_some_and(|(t, _, _)| *t < elapsed - 5.0) {
                            fight.spread.pop_front();
                        }
                    }
                }
            }
            "SPELL_HEAL" | "SPELL_PERIODIC_HEAL" => {
                let amount = num(f.get(f.len().checked_sub(5)?));
                let over = num(f.get(f.len().checked_sub(3)?));
                if let Some((guid, name)) = credited(src, src_name, src_flags) {
                    let s = stats(&mut fight.players, guid.clone(), name);
                    s.healing += amount.saturating_sub(over);
                    s.overhealing += over;
                    let critical = crit(f.last());
                    fight.tallies.entry(guid).or_default().add(elapsed, spell_of(&f), true, !is_player(src), amount.saturating_sub(over), over, critical);
                }
                if is_player(dst) && grouped(dst_flags) {
                    let (spell_id, spell) = spell_of(&f);
                    let hp = fight.hp(dst, elapsed);
                    fight.remember(dst, RecapEvent {
                        at: elapsed,
                        kind: "heal".into(),
                        source: short(src_name),
                        spell,
                        spell_id,
                        amount: amount.saturating_sub(over),
                        overheal: over,
                        crit: crit(f.last()),
                        hp,
                        ..Default::default()
                    });
                }
            }
            "SPELL_AURA_APPLIED" | "SPELL_AURA_REMOVED"
                if is_player(dst) && grouped(dst_flags) && !is_player(src) && !pets.contains_key(src) && f.get(12).is_some_and(|t| t == "DEBUFF") =>
            {
                let (spell_id, spell) = spell_of(&f);
                let kind = if event == "SPELL_AURA_APPLIED" { "debuff" } else { "debuffEnd" };
                fight.remember(dst, RecapEvent { at: elapsed, kind: kind.into(), source: short(src_name), spell, spell_id, ..Default::default() });
            }
            "SPELL_ABSORBED" => {
                // … lanceur du bouclier (GUID, nom, drapeaux ×2), sort (×3), montant, montant de base, critique.
                let n = f.len();
                if n >= 18 {
                    let (guid, name, flags) = (&f[n - 10], &f[n - 9], &f[n - 8]);
                    let pet = !is_player(guid);
                    if let Some((guid, name)) = credited(guid, name, flags) {
                        let amount = num(f.get(n - 3));
                        stats(&mut fight.players, guid.clone(), name).absorbs += amount;
                        let spell = (f[n - 6].parse().unwrap_or(0), f[n - 5].clone());
                        fight.tallies.entry(guid).or_default().add(elapsed, spell, true, pet, amount, 0, crit(f.get(n - 1)));
                    }
                }
            }
            "SPELL_CAST_SUCCESS" | "SPELL_CAST_START" if fight.casts.len() < MAX_CASTS => {
                // Joueurs : le sort parti seulement ; boss et adds : aussi le début de l'incantation, à surveiller.
                let player = is_player(src) && grouped(src_flags) && event == "SPELL_CAST_SUCCESS";
                let enemy = !is_player(src) && !pets.contains_key(src) && hostile(src_flags) && src_name != "nil";
                if player || enemy {
                    let (spell_id, spell) = spell_of(&f);
                    let target = if dst_name == "nil" { String::new() } else { short(dst_name) };
                    let school = f.get(11).and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0);
                    fight.casts.push(Cast { at: elapsed, source: short(src_name), spell_id, spell, target, hostile: enemy, start: event == "SPELL_CAST_START", school });
                }
            }
            "UNIT_DIED" if !is_player(dst) => {
                if let Some(track) = fight.enemies.get_mut(dst.as_str()) {
                    track.died = true;
                }
            }
            "UNIT_DIED" if is_player(dst) && grouped(dst_flags) => {
                stats(&mut fight.players, dst.clone(), short(dst_name)).deaths += 1;
                let hits = fight.recent.remove(dst).map(Vec::from).unwrap_or_default();
                let mut recap: Vec<RecapEvent> = fight.recap.remove(dst).map(Vec::from).unwrap_or_default();
                for e in recap.iter_mut().filter(|e| e.kind == "damage" && e.spell_id != 0) {
                    let mut hit: Vec<&String> = fight
                        .spread
                        .iter()
                        .filter(|(t, id, _)| *id == e.spell_id && (t - e.at).abs() <= SPREAD_SECONDS)
                        .map(|(_, _, who)| who)
                        .collect();
                    hit.sort();
                    hit.dedup();
                    e.aoe = hit.len() as u64;
                }
                let max_hp = fight.health.get(dst).map(|(_, max, _)| *max);
                fight.deaths.push(Death { name: short(dst_name), at: elapsed, hits, recap, max_hp });
            }
            _ => {}
        }
        None
    }
}

/// Un journal de combat sur le disque, proposé à l'envoi à la main.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    pub path: String,
    /// Version du jeu (_classic_beta_…).
    pub flavor: String,
    /// Secondes depuis 1970.
    pub modified: i64,
    pub size: u64,
}

/// Journaux de combat de toutes les versions du jeu installées côte à côte (Bêta,
/// Stress Test…), du plus récent au plus ancien, écrits depuis `max_age`.
fn logs(saved_variables: &Path, max_age: Duration) -> Vec<(PathBuf, String, SystemTime, u64)> {
    let since = SystemTime::now() - max_age;
    let mut found: Vec<_> = crate::flavors::siblings(saved_variables)
        .into_iter()
        .flat_map(|flavor| {
            let name = flavor.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            fs::read_dir(flavor.join("Logs"))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| {
                    let file = e.file_name().to_string_lossy().to_string();
                    file.starts_with("WoWCombatLog") && file.ends_with(".txt")
                })
                .filter_map(move |e| {
                    let meta = e.metadata().ok()?;
                    Some((e.path(), name.clone(), meta.modified().ok()?, meta.len()))
                })
                .collect::<Vec<_>>()
        })
        .filter(|(_, _, modified, _)| *modified > since)
        .collect();
    found.sort_by(|a, b| b.2.cmp(&a.2));
    found
}

/// Journal le plus récent, s'il a été écrit dans les 12 dernières heures.
fn newest(saved_variables: &Path) -> Option<PathBuf> {
    logs(saved_variables, Duration::from_secs(12 * 3600)).into_iter().next().map(|(path, ..)| path)
}

/// Journaux des 30 derniers jours, pour un envoi à la main.
pub fn recent_logs(saved_variables: &Path) -> Vec<LogFile> {
    logs(saved_variables, Duration::from_secs(30 * 86400))
        .into_iter()
        .map(|(path, flavor, modified, size)| LogFile {
            path: path.to_string_lossy().to_string(),
            flavor,
            modified: modified.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
            size,
        })
        .collect()
}

/// Tous les combats de boss d'un journal, lu ligne à ligne (il peut être très gros).
pub fn read_all(path: &Path) -> Result<Vec<Encounter>, String> {
    let file = fs::File::open(path).map_err(|e| format!("Journal illisible : {e}"))?;
    let mut reader = BufReader::with_capacity(1 << 20, file);
    let mut parser = Parser::default();
    let mut found = Vec::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(encounter) = parser.feed(String::from_utf8_lossy(&line).trim_end()) {
                    found.push(encounter);
                }
            }
            Err(e) => return Err(format!("Journal illisible : {e}")),
        }
    }
    found.extend(parser.flush());
    Ok(found)
}

/// Envoie chaque combat d'un journal ; renvoie le compte rendu à afficher.
pub fn send_file(site: &str, token: &str, path: &Path) -> Result<String, String> {
    let encounters = read_all(path)?;
    if encounters.is_empty() {
        return Ok("Aucun combat dans ce journal.".into());
    }
    // Par envois groupés (40 combats au plus), pour ne pas dépasser la limite du site.
    let (mut created, mut known) = (0, 0);
    for range in api::batches(&encounters) {
        let done = api::upload_encounters(site, token, &encounters[range])?;
        created += done.created;
        known += done.known;
    }
    let trash = encounters.iter().filter(|e| e.trash).count();
    Ok(format!(
        "{} combat{} dans ce journal ({} de boss, {} de trash) : {} envoyé{}{}.",
        encounters.len(),
        if encounters.len() > 1 { "s" } else { "" },
        encounters.len() - trash,
        trash,
        created,
        if created > 1 { "s" } else { "" },
        if known > 0 { format!(", {known} déjà sur le site") } else { String::new() }
    ))
}

struct Reader {
    path: PathBuf,
    offset: u64,
    rest: String,
    parser: Parser,
    /// Lectures de suite sans rien de nouveau.
    idle: u32,
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
        let mut done: Vec<Encounter> = text[..complete].lines().filter_map(|line| self.parser.feed(line)).collect();
        done.extend(self.parser.ready.drain(..));
        // Plus rien d'écrit depuis 15 s : le pack de trash en cours est fini.
        if complete == 0 {
            self.idle += 1;
            if self.idle >= 3 {
                done.extend(self.parser.flush());
            }
        } else {
            self.idle = 0;
        }
        done
    }
}

/// Surveille le journal de combat toutes les 5 secondes et envoie chaque combat de boss terminé.
pub fn watch(app: AppHandle, state: Shared, config_dir: PathBuf) {
    thread::spawn(move || {
        let mut reader: Option<Reader> = None;
        let mut pending: Vec<Encounter> = Vec::new();
        // Premier pack de trash en attente : il part avec le boss suivant, ou au bout de 3 minutes.
        let mut waiting: Option<std::time::Instant> = None;
        // Trop de demandes : on attend avant de réessayer.
        let mut pause_until: Option<std::time::Instant> = None;
        let mut marker: Option<(String, bool, bool)> = None;
        let mut tick: u32 = 0;
        let mut warned: Option<String> = None;
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
                if let Ok(standings) = api::standings(&settings.site_url, &settings.token) {
                    let _ = crate::addon::write_standings(Path::new(&settings.saved_variables), &standings.code);
                }
            }
            // Évènements à inviter (organisateur, Conseil) : au démarrage puis toutes les 5 minutes ;
            // le jeu les lit à la connexion ou au /reload.
            if settings.ready() && tick % 60 == 1 {
                if let Ok(events) = api::events(&settings.site_url, &settings.token) {
                    let _ = crate::addon::write_events(Path::new(&settings.saved_variables), &events.code);
                }
            }
            // Toutes les minutes : a-t-on joué dans une autre version du jeu (Bêta, Stress Test…) ?
            if settings.ready() && tick % 12 == 0 {
                follow(&app, &state, &config_dir, &mut warned);
            }
            if !settings.ready() || !settings.upload_combat_logs {
                continue;
            }
            let Some(path) = newest(Path::new(&settings.saved_variables)) else { continue };
            if reader.as_ref().map_or(true, |r| r.path != path) {
                reader = Some(Reader { path, offset: 0, rest: String::new(), parser: Parser::default(), idle: 0 });
            }
            pending.extend(reader.as_mut().unwrap().read());

            if !pending.is_empty() && waiting.is_none() {
                waiting = Some(std::time::Instant::now());
            }
            // Le trash d'une instance part avec le boss qui suit, en un seul envoi : on attend un boss,
            // 3 minutes, ou assez de combats pour un envoi plein.
            let due = pending.iter().any(|e| !e.trash)
                || waiting.is_some_and(|t| t.elapsed() >= Duration::from_secs(180))
                || pending.len() >= 40;
            let paused = pause_until.is_some_and(|t| std::time::Instant::now() < t);
            while due && !paused && !pending.is_empty() {
                let range = api::batches(&pending).into_iter().next().unwrap_or(0..pending.len());
                let batch: Vec<Encounter> = pending[range.clone()].to_vec();
                let (bosses, trash) = (batch.iter().filter(|e| !e.trash).count(), batch.iter().filter(|e| e.trash).count());
                let last = batch.iter().rev().find(|e| !e.trash).or(batch.last()).cloned();
                let report = match api::upload_encounters(&settings.site_url, &settings.token, &batch) {
                    Ok(done) => SyncReport {
                        at: chrono::Utc::now().timestamp(),
                        ok: done.failed == 0,
                        message: format!(
                            "Envoyé : {}{}{}{}.",
                            match (bosses, trash) {
                                (0, t) => format!("{t} pack{} de trash", if t > 1 { "s" } else { "" }),
                                (b, 0) => format!("{b} combat{} de boss", if b > 1 { "s" } else { "" }),
                                (b, t) => format!("{b} combat{} de boss et {t} pack{} de trash", if b > 1 { "s" } else { "" }, if t > 1 { "s" } else { "" }),
                            },
                            last.as_ref().filter(|e| !e.trash).map(|e| format!(" ({}, {})", e.name, if e.success { "tué" } else { "essai" })).unwrap_or_default(),
                            if done.known > 0 { format!(", {} déjà reçu{} d’un autre joueur", done.known, if done.known > 1 { "s" } else { "" }) } else { String::new() },
                            if done.failed > 0 { format!(", {} refusé{}", done.failed, if done.failed > 1 { "s" } else { "" }) } else { String::new() },
                        ),
                        character: None,
                        url: done.results.iter().rev().find_map(|r| r.url.clone()),
                        export_at: last.as_ref().map(|e| e.started_at),
                    },
                    Err(message) => {
                        let report = SyncReport { at: chrono::Utc::now().timestamp(), ok: false, message: message.clone(), character: None, url: None, export_at: last.as_ref().map(|e| e.started_at) };
                        if api::retryable(&message) {
                            // Rien n'est perdu : retenté plus tard (une minute après « trop de demandes »).
                            if message.starts_with("Trop de demandes") {
                                pause_until = Some(std::time::Instant::now() + Duration::from_secs(60));
                            }
                            save(&app, &state, &config_dir, report);
                            break;
                        }
                        report
                    }
                };
                pending.drain(range);
                save(&app, &state, &config_dir, report);
                if pending.is_empty() {
                    waiting = None;
                }
            }
            // Site injoignable longtemps : on garde au plus les 500 combats les plus anciens.
            pending.truncate(500);
        }
    });
}

/// Suit la version du jeu où l'on vient de jouer, si l'addon Symposium y est installé :
/// la fiche, la mise à jour de l'addon et le journal de combat automatique y passent.
fn follow(app: &AppHandle, state: &Shared, config_dir: &Path, warned: &mut Option<String>) {
    let current = state.lock().unwrap().saved_variables.clone();
    let Some(played) = crate::flavors::played_elsewhere(Path::new(&current)) else { return };
    let label = crate::flavors::label(&played.flavor);
    if !played.has_addon {
        // Rien à suivre sans l'addon : on le signale une fois.
        if warned.as_deref() != Some(played.flavor.as_str()) {
            *warned = Some(played.flavor.clone());
            let _ = app
                .notification()
                .builder()
                .title("Symposium")
                .body(format!("Tu as joué sur {label}, où l’addon Symposium n’est pas installé : installe-le dans ce dossier, Symposium le suivra tout seul."))
                .show();
        }
        return;
    }
    {
        let mut s = state.lock().unwrap();
        s.saved_variables = played.saved_variables.to_string_lossy().to_string();
        s.last_export_at = None;
        let _ = s.save(config_dir);
    }
    let message = format!("Tu joues maintenant sur {label} : Symposium suit ce dossier (fiche, addon, journal de combat). Fais /reload en jeu.");
    let _ = app.notification().builder().title("Symposium").body(&message).show();
    save(app, state, config_dir, SyncReport { at: chrono::Utc::now().timestamp(), ok: true, message, character: None, url: None, export_at: None });
    let _ = app.emit("settings", ());
    let app = app.clone();
    thread::spawn(move || {
        let _ = crate::update::update_addon(&app, false);
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
10/9/2026 07:53:27.0000  SPELL_CAST_SUCCESS,Player-4620-008EF1C4,"Lame-ClassicBetaPvE2-",0x511,0x80000000,Creature-0-4615-0-6503-1417-0000487521,"Jeune crocilisque des Paluns",0x10a48,0x80000000,1759,"Attaque pernicieuse",0x1,Player-4620-008EF1C4,0000000000000000,599,691,74,0,897,0,0,0,1,0,0,0,-3177.48,-1756.39,1437,6.0284,21
10/9/2026 07:53:28.0000  SPELL_DAMAGE,Player-4620-008EF1C4,"Lame-ClassicBetaPvE2-",0x511,0x80000000,Creature-0-4615-0-6503-1417-0000487521,"Jeune crocilisque des Paluns",0x10a48,0x80000000,1759,"Attaque pernicieuse",0x1,Creature-0-4615-0-6503-1417-0000487521,0000000000000000,599,691,74,0,897,0,0,0,1,0,0,0,-3177.48,-1756.39,1437,6.0284,21,96,126,-1,1,0,0,0,1,nil,nil,ST
10/9/2026 07:53:29.0000  SPELL_CAST_START,Creature-0-4615-0-6503-1040-0000487CC3,"Rampant des tourbières",0xa48,0x80000000,0000000000000000,nil,0x80000000,0x80000000,5678,"Morsure",0x1
10/9/2026 07:53:29.5000  SPELL_CAST_START,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,0000000000000000,nil,0x80000000,0x80000000,2050,"Soins inférieurs",0x2
10/9/2026 07:53:47.1982  SPELL_ABSORBED,Creature-0-4615-0-6503-1040-0000487CC3,"Rampant des tourbières",0xa48,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,3747,"Mot de pouvoir : Bouclier",0x2,31,35,nil
10/9/2026 07:53:48.0000  SPELL_HEAL,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,2050,"Soins inférieurs",0x2,Player-4620-01548E6A,0000000000000000,800,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,120,120,20,0,nil
10/9/2026 07:53:50.0000  SPELL_DAMAGE,Creature-0-4615-0-6503-1040-0000487CC3,"Rampant des tourbières",0xa48,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,5678,"Morsure",0x1,Creature-0-4615-0-6503-1040-0000487CC3,0000000000000000,768,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,900,900,37,1,0,0,0,nil,nil,nil,ST
10/9/2026 07:53:50.1000  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Player-4620-01548E6A,"Soin-ClassicBetaPvE2-",0x512,0x80000000,0
10/9/2026 07:54:12.0000  ENCOUNTER_END,1144,"Grub",1,5,1,60000"#;

    // Trash dans une instance : un pack se termine après 8 s sans coup échangé avec un monstre.
    const TRASH: &str = r#"10/9/2026 07:59:59.0000  ZONE_CHANGE,36,"Les Mortemines",1
10/9/2026 08:00:00.0000  SWING_DAMAGE,Player-4620-008EF1C4,"Lame-X-",0x511,0x80000000,Creature-0-1-0-1-100-00000000AA,"Gardien",0xa48,0x80000000,Player-4620-008EF1C4,0000000000000000,768,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,500,500,-1,1,0,0,0,nil,nil,nil
10/9/2026 08:00:01.0000  SWING_DAMAGE,Creature-0-1-0-1-100-00000000AA,"Gardien",0xa48,0x80000000,Player-4620-008EF1C4,"Lame-X-",0x511,0x80000000,Creature-0-1-0-1-100-00000000AA,0000000000000000,768,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,40,40,-1,1,0,0,0,nil,nil,nil
10/9/2026 08:00:02.0000  SWING_DAMAGE,Player-4620-008EF1C4,"Lame-X-",0x511,0x80000000,Creature-0-1-0-1-100-00000000BB,"Gardien",0xa48,0x80000000,Player-4620-008EF1C4,0000000000000000,768,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,300,300,-1,1,0,0,0,nil,nil,nil
10/9/2026 08:00:03.0000  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Creature-0-1-0-1-100-00000000AA,"Gardien",0xa48,0x80000000,0
10/9/2026 08:00:20.0000  SWING_DAMAGE,Player-4620-008EF1C4,"Lame-X-",0x511,0x80000000,Creature-0-1-0-1-101-00000000CC,"Rat",0xa48,0x80000000,Player-4620-008EF1C4,0000000000000000,768,863,84,0,1002,0,0,0,1,0,0,0,-3116.69,-1677.83,1437,2.1961,24,10,10,-1,1,0,0,0,nil,nil,nil
10/9/2026 08:00:21.0000  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Creature-0-1-0-1-101-00000000CC,"Rat",0xa48,0x80000000,0"#;

    #[test]
    fn decoupe_le_trash_en_packs() {
        let mut parser = Parser::default();
        let mut found: Vec<Encounter> = TRASH.lines().filter_map(|l| parser.feed(l)).collect();
        found.extend(parser.flush());
        assert_eq!(found.len(), 2);
        let first = &found[0];
        assert!(first.trash);
        assert_eq!((first.encounter_id, first.name.as_str(), first.success, first.duration, first.group_size), (0, "Gardien ×2", false, 2.0, 1));
        assert_eq!(first.zone.as_deref(), Some("Les Mortemines"));
        assert_eq!(first.enemies, vec![Enemy { name: "Gardien".into(), count: 2, killed: 1, damage_taken: 800, damage_done: 40 }]);
        assert_eq!(first.players[0].damage, 800);
        assert_eq!((found[1].name.as_str(), found[1].success), ("Rat", true));

        // Hors instance (Les Paluns) : pas de trash.
        let mut outside = Parser::default();
        let none: Vec<Encounter> = TRASH.replace("ZONE_CHANGE,36,", "ZONE_CHANGE,0,").lines().filter_map(|l| outside.feed(l)).collect();
        assert!(none.is_empty() && outside.flush().is_empty());
    }

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
        assert_eq!(by("Lame").damage, 144);
        // Un joueur hors du groupe n'est pas compté.
        assert!(e.players.iter().all(|p| p.name != "Etranger"));

        let healer = by("Soin");
        assert_eq!((healer.healing, healer.overhealing, healer.absorbs), (100, 20, 31));
        assert_eq!((healer.damage_taken, healer.deaths), (940, 1));
        assert_eq!(e.deaths.len(), 1);
        assert_eq!(e.deaths[0].hits.last().map(|h| (h.spell.as_str(), h.amount)), Some(("Morsure", 900)));
        assert_eq!(e.deaths[0].hits[0].spell, "Mêlée");
        // Récit de la mort : les 15 dernières secondes (le coup de 18 s est trop ancien), soin reçu
        // et coup fatal avec son surplus, points de vie lus dans le journal avancé.
        let recap = &e.deaths[0].recap;
        assert_eq!(recap.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(), vec!["heal", "damage"]);
        assert_eq!((recap[0].amount, recap[0].overheal, recap[0].hp), (100, 20, Some(93)));
        assert_eq!((recap[1].spell.as_str(), recap[1].amount, recap[1].overkill, recap[1].school, recap[1].aoe), ("Morsure", 900, 37, 1, 1));
        assert_eq!(e.deaths[0].max_hp, Some(863));
        // Monstres du combat : le Rampant a frappé le groupe, le crocilisque a été frappé (familier compris).
        assert!(!e.trash);
        assert!(e.enemies.iter().any(|m| m.name == "Rampant des tourbières" && m.damage_done == 940));
        assert!(e.enemies.iter().any(|m| m.name == "Jeune crocilisque des Paluns" && m.damage_taken == 176));

        // Détail : sorts de chaque joueur, avec les critiques, courbe par tranches de 5 s, sorts lancés.
        let detail = |name: &str| e.detail.players.iter().find(|p| p.name == name).cloned().unwrap();
        let lame = detail("Lame");
        assert_eq!(lame.spells.len(), 1);
        assert_eq!((lame.spells[0].id, lame.spells[0].amount, lame.spells[0].hits, lame.spells[0].crits), (1759, 144, 2, 1));
        // Coups à 14,3 s et 16 s du début : troisième et quatrième tranches.
        assert_eq!(lame.damage, vec![0, 0, 48, 96]);
        let pet = e.detail.players.iter().find(|p| p.spells.iter().any(|s| s.pet)).unwrap();
        assert_eq!((pet.spells[0].name.as_str(), pet.spells[0].amount), ("Mêlée", 32));
        let soin = detail("Soin");
        let heal = |n: &str| soin.spells.iter().find(|s| s.name == n).cloned().unwrap();
        assert_eq!((heal("Soins inférieurs").amount, heal("Soins inférieurs").overhealing), (100, 20));
        assert!(heal("Mot de pouvoir : Bouclier").heal);
        assert_eq!(soin.healing.iter().sum::<u64>(), 131);
        let casts: Vec<(&str, &str, bool, bool)> =
            e.detail.casts.iter().map(|c| (c.source.as_str(), c.spell.as_str(), c.hostile, c.start)).collect();
        assert_eq!(casts, vec![("Lame", "Attaque pernicieuse", false, false), ("Rampant des tourbières", "Morsure", true, true)]);
        assert_eq!(e.detail.casts[0].target, "Jeune crocilisque des Paluns");
        assert_eq!((e.detail.casts[0].school, e.detail.casts[0].spell_id), (1, 1759));
        // Dégâts subis : par sort pour le groupe, et la courbe de chacun.
        let taken: Vec<(&str, &str, u64, u32)> = e.detail.taken.iter().map(|t| (t.name.as_str(), t.source.as_str(), t.amount, t.hits)).collect();
        assert_eq!(taken, vec![("Morsure", "Rampant des tourbières", 900, 1), ("Mêlée", "Rampant des tourbières", 40, 1)]);
        assert_eq!(soin.taken.iter().sum::<u64>(), 940);
    }

    #[test]
    fn ignore_ce_qui_est_hors_combat_de_boss() {
        let mut parser = Parser::default();
        let outside = LOG.lines().filter(|l| !l.contains("ENCOUNTER_"));
        assert!(outside.filter_map(|l| parser.feed(l)).next().is_none());
    }

    #[test]
    fn lit_un_journal_entier_depuis_le_disque() {
        let path = std::env::temp_dir().join(format!("WoWCombatLog-test-{}.txt", std::process::id()));
        fs::write(&path, LOG).unwrap();
        let encounters = read_all(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(encounters.len(), 1);
        assert_eq!(encounters[0].name, "Grub");
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
