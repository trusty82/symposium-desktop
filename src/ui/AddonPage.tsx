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
                <p className="lead">Je garde l’addon à jour pour toi, avec la version stable du site.</p>
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
                            <small>Sur le site</small>
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
                <Switch
                    checked={ctx.settings?.updateAddon ?? true}
                    onChange={() => ctx.save({ updateAddon: !(ctx.settings?.updateAddon ?? true) })}
                    disabled={!ctx.settings}
                    label="Mise à jour automatique de l’addon"
                    hint="Au lancement, puis toutes les 6 heures. L’ancienne version est gardée à côté, au cas où."
                />
            </section>
        </div>
    );
}
