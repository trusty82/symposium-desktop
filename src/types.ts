export interface SyncReport {
    at: number;
    ok: boolean;
    message: string;
    character: string | null;
    url: string | null;
    exportAt: number | null;
}

export interface AddonStatus {
    dir: string | null;
    installed: string | null;
    latest: string | null;
    updateAvailable: boolean;
}

export interface Settings {
    siteUrl: string;
    token: string;
    savedVariables: string;
    lastExportAt: number | null;
    lastSync: SyncReport | null;
    updateAddon: boolean;
    uploadCombatLogs: boolean;
    lastCombat: SyncReport | null;
}

export interface Candidate {
    path: string;
    flavor: string;
    account: string;
}

export interface Me {
    name: string;
    member: boolean;
    characters: { name: string; level: number }[];
}

/** Une ligne du fil d'activité. */
export interface Activity {
    id: string;
    at: number;
    kind: 'fiche' | 'combat' | 'addon' | 'app';
    ok: boolean;
    message: string;
    url?: string | null;
}

export type Page = 'home' | 'link' | 'addon' | 'combat' | 'settings';

export const FLAVORS: Record<string, string> = {
    _classic_beta_: 'WoW: Forever',
    _classic_: 'Classic',
    _classic_era_: 'Classic Era',
    _retail_: 'Retail',
};

/** « il y a 5 min », « hier à 21:30 »… */
export function ago(seconds: number): string {
    const diff = Math.max(0, Date.now() / 1000 - seconds);
    if (diff < 45) return 'à l’instant';
    if (diff < 3600) return `il y a ${Math.round(diff / 60)} min`;
    if (diff < 6 * 3600) return `il y a ${Math.round(diff / 3600)} h`;
    const date = new Date(seconds * 1000);
    const time = date.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
    const today = new Date();
    const yesterday = new Date(Date.now() - 86400000);
    if (date.toDateString() === today.toDateString()) return `aujourd’hui à ${time}`;
    if (date.toDateString() === yesterday.toDateString()) return `hier à ${time}`;
    return `${date.toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long' })} à ${time}`;
}
