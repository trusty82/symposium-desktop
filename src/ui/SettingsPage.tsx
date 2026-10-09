import type { Ctx } from '../App';
import Icon from './Icon';
import Switch from './Switch';

const mac = navigator.userAgent.includes('Mac');

export default function SettingsPage({ ctx }: { ctx: Ctx }) {
    return (
        <div className="page">
            <header className="page-head">
                <p className="eyebrow">Réglages</p>
                <h1>Symposium</h1>
                <p className="lead">Version {ctx.version}, mise à jour toute seule.</p>
            </header>

            <section className="panel">
                <Switch
                    checked={ctx.autostart}
                    onChange={ctx.toggleAutostart}
                    label={`Lancer Symposium au démarrage ${mac ? 'du Mac' : 'de Windows'}`}
                    hint="Discret : il démarre dans la barre, sans ouvrir de fenêtre."
                />
            </section>

            <section className="panel">
                <h2>Mises à jour</h2>
                <p className="dim">Je vérifie au lancement puis toutes les 6 heures, et je m’installe tout seul.</p>
                <div className="row">
                    <button type="button" className="btn primary" onClick={ctx.checkUpdate} disabled={Boolean(ctx.busy)}>
                        <Icon name="refresh" size={18} /> Rechercher maintenant
                    </button>
                    <button type="button" className="btn ghost" onClick={() => ctx.open(ctx.site)}>
                        <Icon name="external" size={18} /> Site de la guilde
                    </button>
                </div>
            </section>

            <section className="panel">
                <h2>Raccourcis</h2>
                <ul className="shortcuts">
                    <li>
                        <kbd>{mac ? '⌘' : 'Ctrl'}</kbd> <kbd>K</kbd> palette de commandes : tout faire au clavier
                    </li>
                    <li>
                        Fermer la fenêtre ne m’arrête pas :{' '}
                        {mac ? 'mon icône reste dans la barre des menus.' : 'mon icône reste près de l’horloge (clic droit pour quitter).'}
                    </li>
                </ul>
            </section>
        </div>
    );
}
