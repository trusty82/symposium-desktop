//! Appels au site (/api/desktop/…) avec le jeton personnel.

use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub name: String,
    pub member: bool,
    pub council: bool,
    pub addon_version: Option<String>,
    /// Bêta de l'addon, seulement si elle est plus récente que la stable (site récent).
    #[serde(default)]
    pub addon_beta_version: Option<String>,
    pub characters: Vec<MeCharacter>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MeCharacter {
    pub name: String,
    pub level: i64,
    pub class: String,
    // Champs ajoutés avec la page Personnages (0.13) : absents d'un site plus ancien.
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub class_color: Option<String>,
    #[serde(default)]
    pub race_name: Option<String>,
    #[serde(default)]
    pub spec: Option<String>,
    #[serde(default)]
    pub item_level: Option<f64>,
    /// Le préféré : celui choisi, sinon le plus avancé.
    #[serde(default)]
    pub main: bool,
    /// Préféré choisi par le joueur (pas seulement le plus avancé).
    #[serde(default)]
    pub main_chosen: bool,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Characters {
    pub characters: Vec<MeCharacter>,
}

#[derive(Deserialize)]
pub struct Uploaded {
    pub name: String,
    pub created: bool,
    pub url: String,
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("SymposiumDesktop/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("client HTTP")
}

fn url(site: &str, path: &str) -> String {
    format!("{}/api/desktop{}", site.trim_end_matches('/'), path)
}

/// Erreur lisible à partir de la réponse du site.
fn error(response: Response) -> String {
    let status = response.status().as_u16();
    let body: serde_json::Value = response.json().unwrap_or_default();
    let detail = body["errors"]["code"][0].as_str().or(body["message"].as_str()).unwrap_or("");
    match status {
        401 => "Jeton invalide ou révoqué : crée-en un nouveau dans ton profil sur le site.".into(),
        403 => "Accès réservé aux membres de Symposium.".into(),
        422 => format!("Export refusé par le site : {detail}"),
        429 => "Trop de demandes : réessaie dans une minute.".into(),
        413 => "Envoi trop gros pour le site.".into(),
        _ => format!("Erreur du site ({status}) {detail}"),
    }
}

pub fn me(site: &str, token: &str) -> Result<Me, String> {
    let response = client().get(url(site, "/me")).bearer_auth(token.trim()).header("Accept", "application/json").send().map_err(|e| format!("Site injoignable : {e}"))?;
    if !response.status().is_success() { return Err(error(response)); }
    response.json().map_err(|e| format!("Réponse du site illisible : {e}"))
}

/// Choisit le personnage préféré du compte ; renvoie la liste mise à jour.
pub fn set_main(site: &str, token: &str, id: i64) -> Result<Characters, String> {
    let response = client()
        .put(url(site, &format!("/characters/{id}/main")))
        .bearer_auth(token.trim())
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("Site injoignable : {e}"))?;
    if response.status().as_u16() == 404 {
        return Err("Personnage introuvable sur ton compte : actualise la liste.".into());
    }
    if !response.status().is_success() { return Err(error(response)); }
    response.json().map_err(|e| format!("Réponse du site illisible : {e}"))
}

pub fn upload(site: &str, token: &str, code: &str) -> Result<Uploaded, String> {
    let response = client()
        .post(url(site, "/characters"))
        .bearer_auth(token.trim())
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "code": code }))
        .send()
        .map_err(|e| format!("Site injoignable : {e}"))?;
    if !response.status().is_success() { return Err(error(response)); }
    response.json().map_err(|e| format!("Réponse du site illisible : {e}"))
}

/// Zip de l'addon (version stable du site).
/// Le zip de l'addon : la bêta si `beta` et qu'il y en a une plus récente, sinon la stable.
pub fn download_addon(site: &str, token: &str, beta: bool) -> Result<Vec<u8>, String> {
    let response = client().get(url(site, if beta { "/addon?canal=beta" } else { "/addon" })).bearer_auth(token.trim()).send().map_err(|e| format!("Site injoignable : {e}"))?;
    if !response.status().is_success() { return Err(error(response)); }
    response.bytes().map(|b| b.to_vec()).map_err(|e| format!("Téléchargement de l’addon interrompu : {e}"))
}

/// Combat d'un envoi groupé : reçu (nouveau ou déjà là), ou refusé.
#[derive(Deserialize)]
pub struct BatchResult {
    #[serde(default)]
    pub created: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchUploaded {
    pub results: Vec<BatchResult>,
    pub created: u32,
    pub known: u32,
    pub failed: u32,
}

/// Erreur passagère (site injoignable, trop de demandes) : l'envoi est retenté plus tard.
pub fn retryable(message: &str) -> bool {
    message.starts_with("Site injoignable") || message.starts_with("Trop de demandes")
}

/// Combats au plus par envoi, et taille au plus d'un envoi (un raid de 40 détaillé pèse lourd).
const BATCH: usize = 40;
const BATCH_BYTES: usize = 3 * 1024 * 1024;

/// Découpe des combats en envois : jusqu'à 40 combats et 3 Mo chacun (un seul combat plus gros part seul).
pub fn batches(encounters: &[crate::combatlog::Encounter]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let (mut start, mut bytes) = (0, 0);
    for (i, e) in encounters.iter().enumerate() {
        let size = serde_json::to_vec(e).map(|v| v.len()).unwrap_or(0);
        if i > start && (i - start >= BATCH || bytes + size > BATCH_BYTES) {
            out.push(start..i);
            start = i;
            bytes = 0;
        }
        bytes += size;
    }
    if start < encounters.len() {
        out.push(start..encounters.len());
    }
    out
}

/// Plusieurs combats en un seul envoi : le trash d'une instance et le boss qui suit (1.5.0).
pub fn upload_encounters(site: &str, token: &str, encounters: &[crate::combatlog::Encounter]) -> Result<BatchUploaded, String> {
    let response = Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("SymposiumDesktop/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("client HTTP")
        .post(url(site, "/encounters/batch"))
        .bearer_auth(token.trim())
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "encounters": encounters }))
        .send()
        .map_err(|e| format!("Site injoignable : {e}"))?;
    if !response.status().is_success() { return Err(error(response)); }
    response.json().map_err(|e| format!("Réponse du site illisible : {e}"))
}

#[derive(Deserialize)]
pub struct Standings {
    pub code: String,
    #[serde(default)]
    pub members: Vec<serde_json::Value>,
}

/// Classement EPGP du site, en code SYMPE1 pour l'addon (membres seulement).
pub fn standings(site: &str, token: &str) -> Result<Standings, String> {
    let response = client().get(url(site, "/epgp/standings")).bearer_auth(token.trim()).header("Accept", "application/json").send().map_err(|e| format!("Site injoignable : {e}"))?;
    if !response.status().is_success() { return Err(error(response)); }
    response.json::<Standings>().map_err(|e| format!("Réponse du site illisible : {e}"))
}

#[derive(Deserialize)]
pub struct Events {
    pub code: String,
}

/// Évènements Discord à venir (les siens, tous pour le Conseil), en code SYMPV1 pour l'addon.
pub fn events(site: &str, token: &str) -> Result<Events, String> {
    let response = client().get(url(site, "/evenements")).bearer_auth(token.trim()).header("Accept", "application/json").send().map_err(|e| format!("Site injoignable : {e}"))?;
    if !response.status().is_success() { return Err(error(response)); }
    response.json::<Events>().map_err(|e| format!("Réponse du site illisible : {e}"))
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PairStart {
    pub code: String,
    pub secret: String,
    pub url: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// « Se connecter avec le site » : un code court, à autoriser sur la page du site ouverte dans le navigateur.
pub fn pair_start(site: &str, device: &str) -> Result<PairStart, String> {
    let response = client()
        .post(url(site, "/pair"))
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "device": device }))
        .send()
        .map_err(|e| format!("Site injoignable : {e}"))?;
    if !response.status().is_success() { return Err(error(response)); }
    response.json().map_err(|e| format!("Réponse du site illisible : {e}"))
}

pub enum Pairing {
    Pending,
    Approved { token: String, name: String },
    Expired,
}

/// Où en est le code : en attente, autorisé (le jeton, remis une seule fois) ou expiré.
pub fn pair_poll(site: &str, secret: &str) -> Result<Pairing, String> {
    let response = client()
        .post(url(site, "/pair/poll"))
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "secret": secret }))
        .send()
        .map_err(|e| format!("Site injoignable : {e}"))?;
    match response.status().as_u16() {
        202 => Ok(Pairing::Pending),
        410 => Ok(Pairing::Expired),
        200 => {
            let body: serde_json::Value = response.json().map_err(|e| format!("Réponse du site illisible : {e}"))?;
            let token = body["token"].as_str().ok_or("Réponse du site sans jeton.")?.to_string();
            Ok(Pairing::Approved { token, name: body["name"].as_str().unwrap_or("").to_string() })
        }
        _ => Err(error(response)),
    }
}
