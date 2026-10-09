import '@fontsource/alegreya-sans/400.css';
import '@fontsource/alegreya-sans/500.css';
import '@fontsource/alegreya-sans/700.css';
import '@fontsource/cinzel/600.css';
import '@fontsource/cinzel/800.css';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useCallback, useEffect, useState } from 'react';
import './App.css';
import { type Activity, type AddonStatus, type Candidate, type Page, type Settings, type SyncReport } from './types';
import AddonPage from './ui/AddonPage';
import CombatPage from './ui/CombatPage';
import Home from './ui/Home';
import LinkPage from './ui/LinkPage';
import Palette, { type Command } from './ui/Palette';
import Rail from './ui/Rail';
import SettingsPage from './ui/SettingsPage';
import Toasts, { type Toast } from './ui/Toasts';

/** Tout ce dont les écrans ont besoin : état et actions. */
export interface Ctx {
    settings: Settings | null;
    addon: AddonStatus | null;
    candidates: Candidate[];
    version: string;
    autostart: boolean;
    busy: string | null;
    activity: Activity[];
    ready: boolean;
    site: string;
    go: (page: Page) => void;
    open: (url: string) => void;
    notify: (message: string, ok?: boolean) => void;
    save: (patch: Partial<Pick<Settings, 'token' | 'savedVariables' | 'updateAddon' | 'uploadCombatLogs'>>) => Promise<boolean>;
    detect: () => Promise<void>;
    syncNow: () => Promise<void>;
    updateAddon: () => Promise<void>;
    checkUpdate: () => Promise<void>;
    toggleAutostart: () => Promise<void>;
    refreshStandings: () => Promise<void>;
}

const fromReport = (kind: Activity['kind'], r: SyncReport): Activity => ({
    id: `${kind}-${r.at}-${r.message}`,
    at: r.at,
    kind,
    ok: r.ok,
    message: r.message,
    url: r.url,
});

export default function App() {
    const [page, setPage] = useState<Page>('home');
    const [settings, setSettings] = useState<Settings | null>(null);
    const [addon, setAddon] = useState<AddonStatus | null>(null);
    const [candidates, setCandidates] = useState<Candidate[]>([]);
    const [version, setVersion] = useState('');
    const [autostart, setAutostart] = useState(false);
    const [busy, setBusy] = useState<string | null>(null);
    const [activity, setActivity] = useState<Activity[]>([]);
    const [toasts, setToasts] = useState<Toast[]>([]);
    const [palette, setPalette] = useState(false);

    const push = useCallback((item: Activity) => {
        setActivity((list) => (list.some((a) => a.id === item.id) ? list : [item, ...list].sort((a, b) => b.at - a.at).slice(0, 30)));
    }, []);

    const notify = useCallback((message: string, ok = true) => {
        const id = Date.now() + Math.random();
        setToasts((list) => [...list, { id, message, ok }]);
        setTimeout(() => setToasts((list) => list.filter((t) => t.id !== id)), 4500);
    }, []);

    const refreshAddon = useCallback(() => {
        invoke<AddonStatus>('addon_status').then(setAddon).catch(() => {});
    }, []);

    useEffect(() => {
        invoke<Settings>('get_settings').then((s) => {
            setSettings(s);
            if (s.lastSync) push(fromReport('fiche', s.lastSync));
            if (s.lastCombat) push(fromReport('combat', s.lastCombat));
        });
        invoke<Candidate[]>('detect_saved_variables').then(setCandidates);
        isEnabled().then(setAutostart).catch(() => {});
        getVersion().then(setVersion);
        refreshAddon();
        // Envois faits en arrière-plan (fin de partie, fin de combat de boss).
        const offSync = listen<SyncReport>('sync', (e) => {
            setSettings((s) => (s ? { ...s, lastSync: e.payload } : s));
            push(fromReport('fiche', e.payload));
        });
        const offCombat = listen<SyncReport>('combat', (e) => {
            setSettings((s) => (s ? { ...s, lastCombat: e.payload } : s));
            push(fromReport('combat', e.payload));
        });
        return () => {
            offSync.then((stop) => stop());
            offCombat.then((stop) => stop());
        };
    }, [push, refreshAddon]);

    // ⌘K / Ctrl+K : palette de commandes.
    useEffect(() => {
        const onKey = (e: KeyboardEvent) => {
            if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
                e.preventDefault();
                setPalette((v) => !v);
            }
        };
        window.addEventListener('keydown', onKey);
        return () => window.removeEventListener('keydown', onKey);
    }, []);

    const site = settings?.siteUrl ?? 'https://symposium-gaming.com';
    const ready = Boolean(settings?.token && settings?.savedVariables);

    const ctx: Ctx = {
        settings,
        addon,
        candidates,
        version,
        autostart,
        busy,
        activity,
        ready,
        site,
        go: setPage,
        open: (url) => void openUrl(url),
        notify,
        save: async (patch) => {
            if (!settings) return false;
            try {
                const next = await invoke<Settings>('save_settings', {
                    token: patch.token ?? settings.token,
                    savedVariables: patch.savedVariables ?? settings.savedVariables,
                    updateAddon: patch.updateAddon,
                    uploadCombatLogs: patch.uploadCombatLogs,
                });
                setSettings(next);
                refreshAddon();
                return true;
            } catch (e) {
                notify(String(e), false);
                return false;
            }
        },
        detect: async () => setCandidates(await invoke<Candidate[]>('detect_saved_variables')),
        syncNow: async () => {
            setBusy('Envoi de ta fiche…');
            try {
                const report = await invoke<SyncReport | null>('sync_now');
                if (report) {
                    setSettings((s) => (s ? { ...s, lastSync: report } : s));
                    push(fromReport('fiche', report));
                    notify(report.message, report.ok);
                }
            } finally {
                setBusy(null);
            }
        },
        updateAddon: async () => {
            setBusy('Mise à jour de l’addon…');
            try {
                const installed = await invoke<string | null>('update_addon_now');
                const message = installed ? `Addon mis à jour en ${installed} : fais /reload en jeu.` : 'L’addon est déjà à jour.';
                notify(message);
                if (installed) push({ id: `addon-${Date.now()}`, at: Date.now() / 1000, kind: 'addon', ok: true, message });
                refreshAddon();
            } catch (e) {
                notify(String(e), false);
            } finally {
                setBusy(null);
            }
        },
        checkUpdate: async () => {
            setBusy('Recherche d’une mise à jour…');
            try {
                // Une mise à jour trouvée s'installe et relance l'application.
                const found = await invoke<string | null>('check_update');
                notify(found ? `Installation de la version ${found}…` : `Symposium ${version} est à jour.`);
            } catch (e) {
                notify(String(e), false);
            } finally {
                setBusy(null);
            }
        },
        refreshStandings: async () => {
            setBusy('Récupération du classement EPGP…');
            try {
                const message = await invoke<string>('refresh_standings');
                notify(message);
                push({ id: `epgp-${Date.now()}`, at: Date.now() / 1000, kind: 'addon', ok: true, message });
            } catch (e) {
                notify(String(e), false);
            } finally {
                setBusy(null);
            }
        },
        toggleAutostart: async () => {
            if (autostart) await disable();
            else await enable();
            setAutostart(await isEnabled());
        },
    };

    const commands: Command[] = [
        { id: 'sync', label: 'Envoyer ma fiche maintenant', icon: 'send', run: ctx.syncNow, disabled: !ready },
        { id: 'addon', label: 'Mettre à jour l’addon', icon: 'addon', run: ctx.updateAddon, disabled: !addon?.updateAvailable },
        { id: 'epgp', label: 'Récupérer le classement EPGP maintenant', icon: 'refresh', run: ctx.refreshStandings, disabled: !ready },
        { id: 'update', label: 'Rechercher une mise à jour de Symposium', icon: 'refresh', run: ctx.checkUpdate },
        { id: 'site', label: 'Ouvrir le site de la guilde', icon: 'external', run: () => ctx.open(site) },
        { id: 'profil', label: 'Ouvrir mon profil (jeton)', icon: 'external', run: () => ctx.open(`${site}/profil#application`) },
        { id: 'calendar', label: 'Ouvrir le calendrier des sorties', icon: 'external', run: () => ctx.open(`${site}/calendrier`) },
        { id: 'go-home', label: 'Aller à l’accueil', icon: 'home', run: () => setPage('home') },
        { id: 'go-link', label: 'Aller à la liaison (jeton, fichier)', icon: 'link', run: () => setPage('link') },
        { id: 'go-addon', label: 'Aller à l’addon', icon: 'addon', run: () => setPage('addon') },
        { id: 'go-combat', label: 'Aller aux combats', icon: 'combat', run: () => setPage('combat') },
        { id: 'go-settings', label: 'Aller aux réglages', icon: 'settings', run: () => setPage('settings') },
    ];

    return (
        <div className="shell">
            <div className="aurora" aria-hidden="true" />
            <Rail page={page} go={setPage} ready={ready} addonAlert={Boolean(addon?.updateAvailable)} onPalette={() => setPalette(true)} />
            <main className="stage" key={page}>
                {page === 'home' && <Home ctx={ctx} />}
                {page === 'link' && <LinkPage ctx={ctx} />}
                {page === 'addon' && <AddonPage ctx={ctx} />}
                {page === 'combat' && <CombatPage ctx={ctx} />}
                {page === 'settings' && <SettingsPage ctx={ctx} />}
            </main>
            {palette && <Palette commands={commands} onClose={() => setPalette(false)} />}
            <Toasts toasts={toasts} />
        </div>
    );
}
