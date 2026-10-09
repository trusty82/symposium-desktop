import type { Ctx } from '../App';
import logo from '../assets/logo.png';
import { type Activity, ago } from '../types';
import Icon, { type IconName } from './Icon';

type State = 'setup' | 'ok' | 'busy' | 'error';

const STATES: Record<State, { title: string; text: string }> = {
    setup: { title: 'À éveiller', text: 'Relie l’application à ton compte et au jeu pour commencer.' },
    ok: { title: 'En veille', text: 'Je surveille ton personnage et tes combats. Joue, je m’occupe du reste.' },
    busy: { title: 'À l’œuvre', text: '' },
    error: { title: 'Un souci', text: 'Le dernier envoi n’a pas abouti : détails ci-dessous.' },
};

const KIND: Record<Activity['kind'], { icon: IconName; label: string }> = {
    fiche: { icon: 'send', label: 'Fiche' },
    combat: { icon: 'combat', label: 'Combat' },
    addon: { icon: 'addon', label: 'Addon' },
    app: { icon: 'spark', label: 'Symposium' },
};

/** L'orbe : l'état de l'application d'un coup d'œil. */
function Orb({ state }: { state: State }) {
    return (
        <div className={`orb orb-${state}`} aria-hidden="true">
            <div className="orb-ring" />
            <div className="orb-ring orb-ring-2" />
            <div className="orb-core">
                <img src={logo} alt="" />
            </div>
        </div>
    );
}

function Setup({ ctx }: { ctx: Ctx }) {
    const steps = [
        { done: Boolean(ctx.settings?.token), label: 'Ton jeton personnel', hint: 'Créé sur ton profil du site, en un clic.' },
        { done: Boolean(ctx.settings?.savedVariables), label: 'Le fichier de l’addon', hint: 'Je le trouve tout seul dans ton dossier du jeu.' },
        { done: Boolean(ctx.addon?.installed), label: 'L’addon Symposium', hint: 'Installé dans le jeu (CurseForge ou ici même).' },
    ];
    const done = steps.filter((s) => s.done).length;
    return (
        <section className="panel setup">
            <div className="setup-head">
                <h2>Trois pas pour commencer</h2>
                <span className="setup-count">{done}/3</span>
            </div>
            <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={3} aria-valuenow={done}>
                <span style={{ width: `${(done / 3) * 100}%` }} />
            </div>
            <ol className="steps">
                {steps.map((s, i) => (
                    <li key={s.label} className={s.done ? 'done' : ''}>
                        <span className="step-dot">{s.done ? <Icon name="check" size={14} /> : i + 1}</span>
                        <span>
                            <strong>{s.label}</strong>
                            <small>{s.hint}</small>
                        </span>
                    </li>
                ))}
            </ol>
            <button type="button" className="btn primary" onClick={() => ctx.go(ctx.settings?.token && ctx.settings?.savedVariables ? 'addon' : 'link')}>
                Continuer <Icon name="arrow" size={18} />
            </button>
        </section>
    );
}

export default function Home({ ctx }: { ctx: Ctx }) {
    const latest = ctx.activity[0];
    const state: State = !ctx.ready ? 'setup' : ctx.busy ? 'busy' : latest && !latest.ok && Date.now() / 1000 - latest.at < 86400 ? 'error' : 'ok';
    const sync = ctx.settings?.lastSync;
    const combat = ctx.settings?.lastCombat;
    const greeting = new Date().getHours() < 6 || new Date().getHours() >= 18 ? 'Bonsoir' : 'Bonjour';

    return (
        <div className="page home">
            <section className="hero">
                <Orb state={state} />
                <div className="hero-text">
                    <p className="eyebrow">{greeting}, aventurier</p>
                    <h1>{STATES[state].title}</h1>
                    <p className="lead">{state === 'busy' ? ctx.busy : STATES[state].text}</p>
                    {ctx.ready && (
                        <div className="hero-actions">
                            <button type="button" className="btn primary" onClick={ctx.syncNow} disabled={Boolean(ctx.busy)}>
                                <Icon name="send" size={18} /> Envoyer ma fiche
                            </button>
                            <button type="button" className="btn ghost" onClick={() => ctx.open(`${ctx.site}/calendrier`)}>
                                <Icon name="external" size={18} /> Calendrier
                            </button>
                        </div>
                    )}
                </div>
            </section>

            {!ctx.ready ? (
                <Setup ctx={ctx} />
            ) : (
                <>
                    <section className="tiles" aria-label="État">
                        <button type="button" className="tile" onClick={() => (sync?.url ? ctx.open(sync.url) : ctx.go('link'))}>
                            <span className="tile-icon">
                                <Icon name="send" />
                            </span>
                            <span className="tile-label">Fiche</span>
                            <strong>{sync?.character ?? (sync ? (sync.ok ? 'Envoyée' : 'À revoir') : 'En attente')}</strong>
                            <small>{sync ? ago(sync.at) : 'Quitte le jeu une fois'}</small>
                        </button>
                        <button type="button" className="tile" onClick={() => (combat?.url ? ctx.open(combat.url) : ctx.go('combat'))}>
                            <span className="tile-icon">
                                <Icon name="combat" />
                            </span>
                            <span className="tile-label">Combats</span>
                            <strong>{ctx.settings?.uploadCombatLogs ? (combat ? (combat.ok ? 'Reçus' : 'À revoir') : 'Prêt') : 'Coupés'}</strong>
                            <small>{combat ? ago(combat.at) : 'Au prochain donjon'}</small>
                        </button>
                        <button type="button" className={`tile${ctx.addon?.updateAvailable ? ' tile-glow' : ''}`} onClick={() => ctx.go('addon')}>
                            <span className="tile-icon">
                                <Icon name="addon" />
                            </span>
                            <span className="tile-label">Addon</span>
                            <strong>{ctx.addon?.installed ?? 'Absent'}</strong>
                            <small>{ctx.addon?.updateAvailable ? `${ctx.addon.latest} disponible` : ctx.addon?.installed ? 'À jour' : 'À installer'}</small>
                        </button>
                    </section>

                    <section className="panel feed" aria-labelledby="feed-title">
                        <h2 id="feed-title">Chronique</h2>
                        {ctx.activity.length === 0 ? (
                            <p className="dim">Rien encore. Ta prochaine fin de partie ou ton prochain boss s’inscriront ici.</p>
                        ) : (
                            <ol className="timeline">
                                {ctx.activity.slice(0, 8).map((a) => (
                                    <li key={a.id} className={a.ok ? '' : 'bad'}>
                                        <span className="timeline-dot">
                                            <Icon name={KIND[a.kind].icon} size={14} />
                                        </span>
                                        <span className="timeline-body">
                                            <span>{a.message}</span>
                                            <small>
                                                {KIND[a.kind].label} · {ago(a.at)}
                                                {a.url && (
                                                    <>
                                                        {' · '}
                                                        <button type="button" className="link" onClick={() => ctx.open(a.url!)}>
                                                            voir
                                                        </button>
                                                    </>
                                                )}
                                            </small>
                                        </span>
                                    </li>
                                ))}
                            </ol>
                        )}
                    </section>
                </>
            )}
        </div>
    );
}
