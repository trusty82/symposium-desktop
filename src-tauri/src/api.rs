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
    pub characters: Vec<MeCharacter>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MeCharacter {
    pub name: String,
    pub level: i64,
    pub class: String,
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
        _ => format!("Erreur du site ({status}) {detail}"),
    }
}

pub fn me(site: &str, token: &str) -> Result<Me, String> {
    let response = client().get(url(site, "/me")).bearer_auth(token.trim()).header("Accept", "application/json").send().map_err(|e| format!("Site injoignable : {e}"))?;
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
