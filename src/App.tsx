import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import logo from "./assets/logo.png";
import "./App.css";

interface SyncReport {
  at: number;
  ok: boolean;
  message: string;
  character: string | null;
  url: string | null;
  exportAt: number | null;
}

interface AddonStatus {
  dir: string | null;
  installed: string | null;
  latest: string | null;
  updateAvailable: boolean;
}

interface Settings {
  siteUrl: string;
  token: string;
  savedVariables: string;
  lastExportAt: number | null;
  lastSync: SyncReport | null;
  updateAddon: boolean;
  uploadCombatLogs: boolean;
  lastCombat: SyncReport | null;
}

interface Candidate {
  path: string;
  flavor: string;
  account: string;
}

interface Me {
  name: string;
  member: boolean;
  characters: { name: string; level: number }[];
}

const FLAVORS: Record<string, string> = {
  _classic_beta_: "WoW: Forever (bêta)",
  _classic_: "Classic",
  _classic_era_: "Classic Era",
  _retail_: "Retail",
};

const when = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString("fr-FR", {
    weekday: "long",
    day: "numeric",
    month: "long",
    hour: "2-digit",
    minute: "2-digit",
  });

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [token, setToken] = useState("");
  const [path, setPath] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [me, setMe] = useState<Me | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [version, setVersion] = useState("");
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [addon, setAddon] = useState<AddonStatus | null>(null);
  const [addonMessage, setAddonMessage] = useState<string | null>(null);

  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => {
      setSettings(s);
      setToken(s.token);
      setPath(s.savedVariables);
    });
    invoke<Candidate[]>("detect_saved_variables").then(setCandidates);
    isEnabled()
      .then(setAutostart)
      .catch(() => {});
    getVersion().then(setVersion);
    invoke<AddonStatus>("addon_status")
      .then(setAddon)
      .catch(() => {});
    // Envoi automatique (déconnexion du jeu) : l'état se met à jour.
    const unlisten = listen<SyncReport>("sync", (event) =>
      setSettings((s) => (s ? { ...s, lastSync: event.payload } : s)),
    );
    // Combat de boss envoyé depuis le journal de combat.
    const unlistenCombat = listen<SyncReport>("combat", (event) =>
      setSettings((s) => (s ? { ...s, lastCombat: event.payload } : s)),
    );
    return () => {
      unlisten.then((stop) => stop());
      unlistenCombat.then((stop) => stop());
    };
  }, []);

  const site = settings?.siteUrl ?? "https://symposium-gaming.com";

  const test = async () => {
    setBusy(true);
    setError(null);
    setMe(null);
    try {
      setMe(await invoke<Me>("test_connection", { token }));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      setSettings(
        await invoke<Settings>("save_settings", {
          token,
          savedVariables: path,
        }),
      );
      invoke<AddonStatus>("addon_status")
        .then(setAddon)
        .catch(() => {});
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const syncNow = async () => {
    setBusy(true);
    try {
      const report = await invoke<SyncReport | null>("sync_now");
      if (report) setSettings((s) => (s ? { ...s, lastSync: report } : s));
    } finally {
      setBusy(false);
    }
  };

  const checkUpdate = async () => {
    setUpdateStatus("Recherche…");
    try {
      const found = await invoke<string | null>("check_update");
      // Une mise à jour trouvée redémarre l'application : on n'arrive ici que si elle est à jour.
      setUpdateStatus(
        found
          ? `Installation de la version ${found}…`
          : "Symposium est à jour.",
      );
    } catch (e) {
      setUpdateStatus(String(e));
    }
  };

  const updateAddon = async () => {
    setBusy(true);
    setAddonMessage("Mise à jour…");
    try {
      const installed = await invoke<string | null>("update_addon_now");
      setAddonMessage(
        installed
          ? `Addon mis à jour en ${installed} : fais /reload en jeu.`
          : "L’addon est déjà à jour.",
      );
      setAddon(await invoke<AddonStatus>("addon_status"));
    } catch (e) {
      setAddonMessage(String(e));
    } finally {
      setBusy(false);
    }
  };

  const toggleAddonUpdates = async () => {
    if (!settings) return;
    setSettings(
      await invoke<Settings>("save_settings", {
        token: settings.token,
        savedVariables: settings.savedVariables,
        updateAddon: !settings.updateAddon,
      }),
    );
  };

  const toggleCombatLogs = async () => {
    if (!settings) return;
    setSettings(
      await invoke<Settings>("save_settings", {
        token: settings.token,
        savedVariables: settings.savedVariables,
        uploadCombatLogs: !settings.uploadCombatLogs,
      }),
    );
  };

  const toggleAutostart = async () => {
    if (autostart) await disable();
    else await enable();
    setAutostart(await isEnabled());
  };

  const dirty =
    settings && (token !== settings.token || path !== settings.savedVariables);
  const ready = settings && settings.token && settings.savedVariables;
  const last = settings?.lastSync;

  return (
    <main className="app">
      <header className="header">
        <img src={logo} alt="" className="logo" />
        <div>
          <h1>Symposium</h1>
          <p className="dim">
            Ta fiche sur le site, mise à jour toute seule quand tu quittes le
            jeu.
          </p>
        </div>
      </header>

      <section className="card">
        <h2>
          <span className="step">1</span> Ton jeton
        </h2>
        <p className="dim">
          Crée-le dans ton profil sur le site (section « Application Symposium
          »), puis colle-le ici.{" "}
          <button
            type="button"
            className="link"
            onClick={() => openUrl(`${site}/profil#application`)}
          >
            Ouvrir mon profil
          </button>
        </p>
        <div className="row">
          <input
            type={showToken ? "text" : "password"}
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="symp_…"
            spellCheck={false}
            aria-label="Jeton personnel"
          />
          <button
            type="button"
            className="ghost"
            onClick={() => setShowToken(!showToken)}
          >
            {showToken ? "Masquer" : "Voir"}
          </button>
          <button
            type="button"
            className="ghost"
            onClick={test}
            disabled={busy || !token}
          >
            Tester
          </button>
        </div>
        {me && (
          <p className="ok">
            ✓ Connecté : <strong>{me.name}</strong>
            {me.characters.length > 0 &&
              ` · ${me.characters.map((c) => `${c.name} (${c.level})`).join(", ")}`}
          </p>
        )}
      </section>

      <section className="card">
        <h2>
          <span className="step">2</span> Le fichier de l’addon
        </h2>
        <p className="dim">
          L’addon Symposium y écrit ton personnage quand tu quittes le jeu ou
          fais /reload.
        </p>
        {candidates.length > 0 ? (
          <ul className="choices">
            {candidates.map((c) => (
              <li key={c.path}>
                <label>
                  <input
                    type="radio"
                    name="sv"
                    checked={path === c.path}
                    onChange={() => setPath(c.path)}
                  />
                  <span>
                    <strong>{FLAVORS[c.flavor] ?? c.flavor}</strong> · compte{" "}
                    {c.account}
                    <span className="path">{c.path}</span>
                  </span>
                </label>
              </li>
            ))}
          </ul>
        ) : (
          <p className="warn">
            Aucun fichier trouvé automatiquement. Connecte-toi une fois en jeu
            avec l’addon Symposium, puis quitte le jeu, ou indique le chemin à
            la main.
          </p>
        )}
        <details>
          <summary>Indiquer le chemin à la main</summary>
          <input
            type="text"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder={
              navigator.userAgent.includes("Mac")
                ? "/Applications/World of Warcraft/_classic_beta_/WTF/Account/…/SavedVariables/Symposium.lua"
                : "C:\\Program Files (x86)\\World of Warcraft\\_classic_beta_\\WTF\\Account\\…\\SavedVariables\\Symposium.lua"
            }
            spellCheck={false}
            aria-label="Chemin du fichier Symposium.lua"
          />
        </details>
        <button
          type="button"
          className="ghost small"
          onClick={() =>
            invoke<Candidate[]>("detect_saved_variables").then(setCandidates)
          }
        >
          Rechercher à nouveau
        </button>
      </section>

      <div className="actions">
        <button
          type="button"
          className="primary"
          onClick={save}
          disabled={busy || !dirty}
        >
          {saved ? "Enregistré ✓" : "Enregistrer"}
        </button>
        {error && <p className="error">{error}</p>}
      </div>

      <section className="card">
        <h2>
          <span className="step">3</span> État
        </h2>
        {!ready ? (
          <p className="dim">
            Renseigne ton jeton et le fichier, puis enregistre.
          </p>
        ) : last ? (
          <p className={last.ok ? "ok" : "error"}>
            {last.ok ? "✓" : "⚠"} {last.message}
            <span className="dim small">
              {" "}
              — {when(last.at)}
              {last.exportAt && ` · export du jeu du ${when(last.exportAt)}`}
            </span>
            {last.url && (
              <>
                {" "}
                <button
                  type="button"
                  className="link"
                  onClick={() => openUrl(last.url!)}
                >
                  Voir la fiche
                </button>
              </>
            )}
          </p>
        ) : (
          <p className="dim">
            En attente : ta fiche partira la prochaine fois que tu quittes le
            jeu (ou fais /reload).
          </p>
        )}
        <div className="row">
          <button
            type="button"
            className="ghost"
            onClick={syncNow}
            disabled={busy || !ready || !!dirty}
          >
            Envoyer ma fiche maintenant
          </button>
        </div>
        <label className="check">
          <input
            type="checkbox"
            checked={autostart}
            onChange={toggleAutostart}
          />
          Lancer Symposium au démarrage de l’ordinateur
        </label>
      </section>

      <section className="card">
        <h2>
          <span className="step">4</span> L’addon
        </h2>
        {!addon?.dir ? (
          <p className="dim">
            Choisis d’abord ton fichier Symposium.lua : l’application en déduit
            le dossier de l’addon.
          </p>
        ) : (
          <p>
            Installé : <strong>{addon.installed ?? "aucun"}</strong>
            {addon.latest && (
              <>
                {" "}
                · sur le site : <strong>{addon.latest}</strong>
              </>
            )}
            {addon.updateAvailable ? (
              <span className="warn"> · mise à jour disponible</span>
            ) : (
              addon.installed && <span className="ok"> · à jour</span>
            )}
          </p>
        )}
        {addonMessage && <p className="dim small">{addonMessage}</p>}
        <div className="row">
          <button
            type="button"
            className="ghost"
            onClick={updateAddon}
            disabled={busy || !ready || !addon?.updateAvailable}
          >
            Mettre à jour l’addon maintenant
          </button>
        </div>
        <label className="check">
          <input
            type="checkbox"
            checked={settings?.updateAddon ?? true}
            onChange={toggleAddonUpdates}
            disabled={!settings}
          />
          Mettre l’addon à jour automatiquement (version stable du site)
        </label>
      </section>

      <section className="card">
        <h2>
          <span className="step">5</span> Journaux de combat
        </h2>
        <p className="dim">
          En donjon et en raid, l’addon Symposium (0.11 et plus) lance tout seul
          le journal de combat, et l’arrête à la sortie. À la fin de chaque
          combat de boss, Symposium envoie au site les dégâts, les soins et les
          morts de chacun ; le journal lui-même reste sur ton ordinateur. Coche
          une fois « Journal de combat avancé » dans les options du jeu
          (Réseau), et fais /reload après avoir activé l’envoi ci-dessous.
        </p>
        {settings?.lastCombat && (
          <p className={settings.lastCombat.ok ? "ok" : "error"}>
            {settings.lastCombat.ok ? "✓" : "⚠"} {settings.lastCombat.message}
            <span className="dim small"> — {when(settings.lastCombat.at)}</span>
            {settings.lastCombat.url && (
              <>
                {" "}
                <button
                  type="button"
                  className="link"
                  onClick={() => openUrl(settings.lastCombat!.url!)}
                >
                  Voir le combat
                </button>
              </>
            )}
          </p>
        )}
        <label className="check">
          <input
            type="checkbox"
            checked={settings?.uploadCombatLogs ?? true}
            onChange={toggleCombatLogs}
            disabled={!settings}
          />
          Journal de combat automatique et envoi des combats de boss au site
        </label>
      </section>

      <p className="footer dim">
        {navigator.userAgent.includes("Mac")
          ? "Fermer cette fenêtre garde Symposium actif : son icône reste dans la barre des menus, en haut de l’écran. Clique dessus pour quitter."
          : "Fermer cette fenêtre garde Symposium actif : son icône reste près de l’horloge. Clic droit dessus pour quitter."}
      </p>
      <p className="footer dim">
        Version {version} · mise à jour automatique ·{" "}
        <button type="button" className="link" onClick={checkUpdate}>
          Rechercher une mise à jour
        </button>
        {updateStatus && <> · {updateStatus}</>}
      </p>
    </main>
  );
}
