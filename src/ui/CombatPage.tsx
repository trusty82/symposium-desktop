import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import type { Ctx } from '../App';
import { ago, FLAVORS, type LogFile } from '../types';
import Icon from './Icon';
import Switch from './Switch';

/** Journaux de combat : lancés tout seuls en instance, résumés envoyés au site. */
export default function CombatPage({ ctx }: { ctx: Ctx }) {
    const last = ctx.settings?.lastCombat;
    const on = ctx.settings?.uploadCombatLogs ?? true;
    const steps = [
        { label: 'Addon Symposium 0.11 ou plus', hint: 'Il lance le journal en entrant dans un donjon ou un raid, et l’arrête à la sortie.' },
        { label: '« Journal de combat avancé » coché', hint: 'Options du jeu > Réseau. Une fois pour toutes.' },
        { label: 'Un /reload après avoir activé l’envoi', hint: 'Pour que le jeu charge le petit addon témoin installé par Symposium.' },
    ];
    return (
        <div className="page">
            <header className="page-head">
                <p className="eyebrow">Combats</p>
                <h1>Chaque boss, raconté</h1>
                <p className="lead">Dégâts, soins, morts : à la fin de chaque combat de boss, le résumé part au site. Le journal reste chez toi.</p>
            </header>

            <section className="panel">
                <Switch
                    checked={on}
                    onChange={() => ctx.save({ uploadCombatLogs: !on })}
                    disabled={!ctx.settings}
                    label="Journal automatique et envoi des combats"
                    hint="Désactivé : l’addon ne lance plus le journal, et rien n’est envoyé."
                />
                {last && (
                    <p className={last.ok ? 'good' : 'bad'}>
                        <Icon name={last.ok ? 'check' : 'alert'} size={16} /> {last.message} <span className="dim">· {ago(last.at)}</span>
                        {last.url && (
                            <>
                                {' '}
                                <button type="button" className="link" onClick={() => ctx.open(last.url!)}>
                                    Voir le combat
                                </button>
                            </>
                        )}
                    </p>
                )}
            </section>

            <OldLogs ctx={ctx} />

            <section className="panel">
                <h2>Pour que ça marche</h2>
                <ol className="checklist">
                    {steps.map((s, i) => (
                        <li key={s.label}>
                            <span className="num">{i + 1}</span>
                            <span>
                                <strong>{s.label}</strong>
                                <small>{s.hint}</small>
                            </span>
                        </li>
                    ))}
                </ol>
                <button type="button" className="btn ghost" onClick={() => ctx.open(`${ctx.site}/calendrier`)}>
                    <Icon name="external" size={18} /> Voir les sorties sur le calendrier
                </button>
            </section>
        </div>
    );
}

const size = (bytes: number) => (bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1).replace('.', ',')} Mo` : `${Math.max(1, Math.round(bytes / 1024))} Ko`);

/** Journaux déjà écrits (Bêta, Stress Test…) : à renvoyer à la main un soir où l'envoi n'a pas eu lieu. */
function OldLogs({ ctx }: { ctx: Ctx }) {
    const [logs, setLogs] = useState<LogFile[] | null>(null);
    const [sending, setSending] = useState<string | null>(null);
    const [results, setResults] = useState<Record<string, { ok: boolean; message: string }>>({});
    const load = () => invoke<LogFile[]>('recent_logs').then(setLogs, () => setLogs([]));
    useEffect(() => {
        load();
    }, [ctx.settings?.savedVariables]);

    const send = async (path: string) => {
        setSending(path);
        try {
            const message = await invoke<string>('send_log', { path });
            setResults((r) => ({ ...r, [path]: { ok: true, message } }));
            ctx.notify(message);
        } catch (e) {
            setResults((r) => ({ ...r, [path]: { ok: false, message: String(e) } }));
            ctx.notify(String(e), false);
        } finally {
            setSending(null);
        }
    };

    return (
        <section className="panel">
            <h2>Anciens journaux</h2>
            <p className="dim">
                Un soir où rien n’est parti (application fermée, autre version du jeu…) : renvoie les combats de boss d’un journal resté sur ton
                disque. Ceux déjà reçus ne sont pas comptés deux fois.
            </p>
            {logs === null ? null : logs.length === 0 ? (
                <p className="dim">Aucun journal de combat ces 30 derniers jours, ni sur la Bêta ni sur les autres versions installées.</p>
            ) : (
                <ul className="logs">
                    {logs.map((l) => (
                        <li key={l.path}>
                            <span>
                                <strong>{ago(l.modified)}</strong> · {FLAVORS[l.flavor] ?? l.flavor} · {size(l.size)}
                                <small>{l.path}</small>
                                {results[l.path] && <small className={results[l.path].ok ? 'good' : 'bad'}>{results[l.path].message}</small>}
                            </span>
                            <button type="button" className="btn small" disabled={!ctx.ready || sending !== null} onClick={() => send(l.path)}>
                                {sending === l.path ? 'Envoi…' : 'Envoyer'}
                            </button>
                        </li>
                    ))}
                </ul>
            )}
            <button type="button" className="link" onClick={load}>
                Actualiser la liste
            </button>
        </section>
    );
}
