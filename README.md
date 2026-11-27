# Symposium (application de bureau)

Relie l'addon WoW: Forever **Symposium** au site de la guilde
[symposium-gaming.com](https://symposium-gaming.com). Application [Tauri](https://tauri.app)
(Rust + React), pour Windows et Mac (universelle : Apple Silicon et Intel).

Installeur Windows signé : voir la [politique de signature du code](CODE_SIGNING_POLICY.md)
(signature gratuite fournie par [SignPath.io](https://signpath.io), certificat de la
[SignPath Foundation](https://signpath.org)).

## Ce qu'elle fait

- Surveille `WTF\Account\<compte>\SavedVariables\Symposium.lua` (écrit par l'addon à la
  déconnexion ou au `/reload`) et envoie le dernier export du personnage (`/symposium`) au site :
  la fiche se met à jour toute seule.
- Se connecte au site par **« Se connecter avec le site »** (1.1.0) : l'application demande un code
  court (`POST /api/desktop/pair`), ouvre la page `/application/lier?code=…` du site où le membre
  autorise l'ordinateur, puis récupère son jeton (`POST /api/desktop/pair/poll`, remis une seule fois).
  Le **jeton personnel** créé dans le profil reste possible en secours ; API `/api/desktop/…` du site.
- Trouve le jeu tout seul (1.1.0) : les fichiers `Symposium.lua` existants d'abord, sinon le compte
  du jeu joué en dernier (WoW: Forever d'abord, jamais Retail, 1.1.1), même si l'addon n'y a encore rien écrit ; l'addon y est alors installé
  (dossier `Interface\AddOns` créé au besoin) dès la liaison.
- Met à jour l'addon Symposium dans le dossier du jeu (version stable du site, désactivable) :
  l'ancien dossier est gardé à côté (`AddOns\.Symposium-ancien`). Vérifiée toutes les 10 minutes
  (0.11.1), et à chaque retour sur la fenêtre pour l'affichage. Case « Addon bêta » (0.10.0) :
  la bêta du site (branche `addon-beta`, `/api/desktop/addon?canal=beta`) quand elle est plus
  récente que la stable ; décochée, retour à la stable.
- Suit la version du jeu où l'on joue (0.11.0) : avec plusieurs versions installées côte à côte
  (Bêta, Stress Test…), l'application passe toute seule sur celle qui a servi en dernier, si
  l'addon Symposium y est installé (`flavors.rs`). Les journaux de combat de toutes ces versions
  sont surveillés, et la page Combats renvoie à la main les combats d'un journal des 30 derniers
  jours (« Anciens journaux »). Détail de chaque combat (0.12.0) : sorts de chaque joueur
  (dégâts, soins, coups, critiques), courbe par tranches de 5 s, sorts lancés par les joueurs, le
  boss et ses adds ; renvoyer un ancien journal complète un combat déjà sur le site. Dégâts subis par
  sort et courbe des dégâts subis (0.12.1). École de magie de chaque sort lancé (0.12.2), pour la frise.
  Récit de chaque mort (1.3.0) : les 15 dernières secondes du joueur (coups reçus avec école,
  surplus, part résistée, bloquée ou absorbée ; soins reçus et soins perdus ; effets posés par les
  ennemis ; points de vie lus dans le journal avancé ; joueurs touchés par le même sort), que le site
  analyse pour en donner la cause.
  Trash (1.4.1) : dans une instance, chaque pack de monstres hors combat de boss (du premier coup
  échangé à 8 s sans combat) est résumé et envoyé comme un combat ; chaque combat, boss ou trash,
  liste aussi ses monstres, hostiles ou neutres (combien, tués, dégâts reçus et infligés au groupe).
  Envois groupés (1.5.0) : les packs de trash attendent le boss suivant (3 minutes au plus) et partent
  avec lui en un seul envoi (`POST /api/desktop/encounters/batch`, 40 combats et 3 Mo au plus) ;
  « trop de demandes » ou site injoignable : rien n'est perdu, l'envoi est retenté.
  Analyse des wipes et combat rejoué (1.6.0) : d'après le journal avancé, la vie de chaque monstre
  (courbe du boss par tranche, vie au wipe), le mana des joueurs par tranche, les interruptions et
  dissipations, les joueurs touchés par chaque sort, et la position et la vie de chaque unité une
  fois par seconde (le site rejoue le combat vu de dessus).
- Page Personnages (0.13.0) : les personnages du compte sur le site, et le choix du préféré
  (`/api/desktop/me`, `PUT /api/desktop/characters/{id}/main`).
- Évènements à inviter (1.2.0) : au démarrage puis toutes les 5 minutes, les évènements Discord à
  venir de `/symposium create` (ceux qu'on organise, tous pour le Conseil), avec le personnage de
  chaque inscrit (`GET /api/desktop/evenements`, code `SYMPV1`), déposés dans l'addon témoin
  (`SymposiumDesktop/Events.lua`) : l'addon Symposium ouvre une fenêtre d'invitations 15 minutes
  avant le début. Lu par le jeu à la connexion ou au `/reload` ; le nouveau fichier demande de
  relancer le jeu une fois.
- Explique quand l'export envoyé est le même que la dernière fois (`/symposium` puis `/reload`).
- Se met à jour toute seule (au démarrage puis toutes les 6 heures) depuis les Releases GitHub
  (`latest.json`, mises à jour signées).
- Version Microsoft Store (0.11.2) : paquet MSIX (`src-tauri/msix/AppxManifest.xml`, job `store`
  du workflow Release, identité de Partner Center dans les variables du dépôt), signé par le
  Store. Compilée avec `--features store` : mises à jour par le Store, démarrage avec Windows
  dans Paramètres › Applications › Démarrage.
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
l'application), `combatlog.rs` (journal de combat), `flavors.rs` (versions du jeu installées), `lib.rs` (fenêtre, zone de notification, commandes). L'interface est dans `src/`.

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

## Licence

[MIT](LICENSE). Signature du code : [CODE_SIGNING_POLICY.md](CODE_SIGNING_POLICY.md).
