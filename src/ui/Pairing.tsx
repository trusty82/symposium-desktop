import { invoke } from '@tauri-apps/api/core';
import { useEffect, useRef, useState } from 'react';
import type { Ctx } from '../App';
import Icon from './Icon';

interface PairStart {
    code: string;
    secret: string;
    url: string;
    expiresIn: number;
    interval: number;
}

interface PairOutcome {
    status: 'pending' | 'approved' | 'expired';
    name: string | null;
}

/**
 * « Se connecter avec le site » : l'application demande un code, ouvre la page du site où le membre
 * autorise l'ordinateur, puis vient chercher son jeton. Plus rien à copier-coller.
 */
export default function Pairing({ ctx, primary = true }: { ctx: Ctx; primary?: boolean }) {
    const [pending, setPending] = useState<PairStart | null>(null);
    const [starting, setStarting] = useState(false);
    const timer = useRef<number | null>(null);

    const stop = () => {
        if (timer.current) window.clearInterval(timer.current);
        timer.current = null;
        setPending(null);
    };

    useEffect(() => stop, []);

    const start = async () => {
        setStarting(true);
        try {
            const p = await invoke<PairStart>('pair_start');
            setPending(p);
            const until = Date.now() + p.expiresIn * 1000;
            let polling = false;
            timer.current = window.setInterval(async () => {
                if (polling) return;
                if (Date.now() > until) {
                    stop();
                    ctx.notify('Le code a expiré : recommence.', false);
                    return;
                }
                polling = true;
                try {
                    const outcome = await invoke<PairOutcome>('pair_poll', {
                        secret: p.secret,
                    });
                    if (outcome.status === 'approved') {
                        stop();
                        await ctx.refresh();
                        ctx.notify(`Connecté en tant que ${outcome.name}. Je m’occupe de l’addon.`);
                    } else if (outcome.status === 'expired') {
                        stop();
                        ctx.notify('Le code a expiré : recommence.', false);
                    }
                } catch {
                    // Coupure réseau passagère : on réessaie au tour suivant.
                } finally {
                    polling = false;
                }
            }, p.interval * 1000);
        } catch (e) {
            ctx.notify(String(e), false);
        } finally {
            setStarting(false);
        }
    };

    if (pending) {
        return (
            <div className="pairing">
                <p className="dim">
                    Une page du site s’est ouverte dans ton navigateur. Vérifie que ce code est le même, puis clique sur Autoriser :
                </p>
                <p className="pair-code" aria-live="polite">
                    {pending.code}
                </p>
                <p className="dim small">
                    <span className="spinner" aria-hidden="true" /> En attente de ton accord…{' '}
                    <button type="button" className="link" onClick={() => ctx.open(pending.url)}>
                        Rouvrir la page
                    </button>{' '}
                    ·{' '}
                    <button type="button" className="link" onClick={stop}>
                        Annuler
                    </button>
                </p>
            </div>
        );
    }

    return (
        <button type="button" className={`btn ${primary ? 'primary' : 'ghost'}`} onClick={start} disabled={starting}>
            <Icon name="link" size={18} />{' '}
            {starting ? 'Ouverture du site…' : ctx.settings?.token ? 'Me reconnecter avec le site' : 'Se connecter avec le site'}
        </button>
    );
}
