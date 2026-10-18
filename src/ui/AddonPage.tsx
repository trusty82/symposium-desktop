import type { Ctx } from '../App';
import Icon from './Icon';
import Switch from './Switch';

/** L'addon Symposium : version installée, version du site, mise à jour. */
export default function AddonPage({ ctx }: { ctx: Ctx }) {
    const a = ctx.addon;
    return (
        <div className="page">
            <header className="page-head">
                <p className="eyebrow">Addon</p>
                <h1>Symposium, en jeu</h1>
                <p className="lead">
                    Je garde l’addon à jour pour toi, avec la version stable du site
                    {ctx.settings?.betaAddon ? ', ou la bêta quand il y en a une' : ''}.
                </p>
            </header>

            <section className="panel">
                {!a?.dir ? (
                    <p className="dim">Choisis d’abord ton fichier dans Liaison : j’en déduis le dossier de l’addon.</p>
                ) : (
                    <div className="versions">
                        <div className="version">
                            <small>Installé</small>
                            <strong>{a.installed ?? '—'}</strong>
                        </div>
                        <div className={`version-arrow${a.updateAvailable ? ' live' : ''}`} aria-hidden="true">
                            <Icon name="arrow" size={28} />
                        </div>
                        <div className="version">
                            <small>{a.latest?.includes('-') ? 'Bêta du site' : 'Sur le site'}</small>
                            <strong>{a.latest ?? '—'}</strong>
                        </div>
                        <p className={`version-state ${a.updateAvailable ? 'warn' : 'good'}`}>
                            {a.updateAvailable ? 'Une mise à jour t’attend.' : a.installed ? 'Tout est à jour.' : 'Pas encore installé.'}
                        </p>
                    </div>
                )}
                <div className="row">
                    <button type="button" className="btn primary" onClick={ctx.updateAddon} disabled={Boolean(ctx.busy) || !ctx.ready || !a?.updateAvailable}>
                        <Icon name="refresh" size={18} /> Mettre à jour maintenant
                    </button>
                    <button type="button" className="btn ghost" onClick={() => ctx.open('https://www.curseforge.com/wow/addons/symposium-guild-companion')}>
                        <Icon name="external" size={18} /> Sur CurseForge
                    </button>
                </div>
                {a?.dir && <p className="path">{a.dir}</p>}
            </section>

            <section className="panel">
                <h2>Classement EPGP</h2>
                <p className="dim">
                    Je récupère ton classement sur le site toutes les 10 minutes et je le dépose pour l’addon : il se charge tout seul à la connexion
                    ou au /reload. Plus de code à copier, et tu vois tes points en jeu.
                </p>
                <div className="row">
                    <button type="button" className="btn ghost" onClick={ctx.refreshStandings} disabled={Boolean(ctx.busy) || !ctx.ready}>
                        <Icon name="refresh" size={18} /> Récupérer le classement maintenant
                    </button>
                </div>
                <p className="dim small">Après une correction sur le site : clique ici, puis /reload en jeu.</p>
            </section>

            <section className="panel">
                <Switch
                    checked={ctx.settings?.updateAddon ?? true}
                    onChange={() => ctx.save({ updateAddon: !(ctx.settings?.updateAddon ?? true) })}
                    disabled={!ctx.settings}
                    label="Mise à jour automatique de l’addon"
                    hint="Au lancement, puis toutes les 6 heures. L’ancienne version est gardée à côté, au cas où."
                />
            </section>

            <section className="panel">
                <Switch
                    checked={ctx.settings?.betaAddon ?? false}
                    onChange={async () => {
                        // Installée tout de suite : on teste sans attendre la prochaine vérification.
                        if (await ctx.save({ betaAddon: !(ctx.settings?.betaAddon ?? false) })) await ctx.updateAddon();
                    }}
                    disabled={!ctx.settings || Boolean(ctx.busy)}
                    label="Addon bêta"
                    hint="Les nouveautés avant leur sortie, pour les tester. Une bêta peut avoir des bugs : signale-les au Conseil. Décoche pour revenir à la version stable."
                />
            </section>
        </div>
    );
}
