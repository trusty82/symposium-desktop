# Symposium (application de bureau)

Relie l'addon WoW: Forever **Symposium** au site de la guilde
[symposium-gaming.com](https://symposium-gaming.com). Application [Tauri](https://tauri.app)
(Rust + React), pour Windows et Mac (universelle : Apple Silicon et Intel).

## Ce qu'elle fait

- Surveille `WTF\Account\<compte>\SavedVariables\Symposium.lua` (écrit par l'addon à la
  déconnexion ou au `/reload`) et envoie le dernier export du personnage (`/symposium`) au site :
  la fiche se met à jour toute seule.
- Se connecte au site avec un **jeton personnel**, créé dans le profil (section
  « Application Symposium ») ; API `/api/desktop/…` du site.
- Met à jour l'addon Symposium dans le dossier du jeu (version stable du site, désactivable) :
  l'ancien dossier est gardé à côté (`AddOns\.Symposium-ancien`).
- Explique quand l'export envoyé est le même que la dernière fois (`/symposium` puis `/reload`).
- Se met à jour toute seule (au démarrage puis toutes les 6 heures) depuis les Releases GitHub
  (`latest.json`, mises à jour signées).
- Tourne dans la zone de notification (fermer la fenêtre la cache), peut se lancer au démarrage
  de Windows (en arrière-plan), une seule instance à la fois.

À venir : déposer le classement EPGP pour l'addon du maître du butin (demande une version bêta de
l'addon).

## Développer

Prérequis : Node 22, Rust (`rustup`), et sur Windows les outils de
[tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev        # application en développement
cargo test --manifest-path src-tauri/Cargo.toml
```

Le code Rust est dans `src-tauri/src` : `savedvars.rs` (fichier de l'addon), `api.rs` (site),
`sync.rs` (surveillance et envoi), `settings.rs` (réglages, dans le dossier de configuration de
l'application), `lib.rs` (fenêtre, zone de notification, commandes). L'interface est dans `src/`.

## Publier

Pousser un tag `vX.Y.Z` (même version que `package.json`, `src-tauri/tauri.conf.json` et
`src-tauri/Cargo.toml`) : le workflow **Release** construit l'installeur Windows
(`Symposium_X.Y.Z_x64-setup.exe`) et l'application Mac universelle (`Symposium_X.Y.Z_universal.dmg`),
puis crée une **Release** GitHub avec les installeurs, les paquets de mise à jour signés et
`latest.json` (Windows et Mac) : les applications installées se mettent à jour d'elles-mêmes.

Signature des mises à jour : clé privée dans les secrets du dépôt (`TAURI_SIGNING_PRIVATE_KEY`,
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), clé publique dans `src-tauri/tauri.conf.json`. Sans la clé
privée, plus aucune mise à jour ne peut être publiée : la garder en lieu sûr.

Aucune application n'est signée par un certificat :
- Windows : SmartScreen affiche « Windows a protégé votre ordinateur » → **Informations
  complémentaires** → **Exécuter quand même**.
- Mac (signature ad hoc) : au premier lancement, clic droit sur Symposium → **Ouvrir** ; sur
  macOS 15 et plus, Réglages Système → Confidentialité et sécurité → **Ouvrir quand même**.
