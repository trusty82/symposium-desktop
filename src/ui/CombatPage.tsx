import type { Ctx } from '../App';
import { ago } from '../types';
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
