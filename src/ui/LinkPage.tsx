import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import type { Ctx } from '../App';
import { FLAVORS, type Me } from '../types';
import Icon from './Icon';
import Pairing from './Pairing';

const mac = navigator.userAgent.includes('Mac');

/** Liaison : le compte (connexion par le site, ou jeton en secours) et le fichier de l'addon. */
export default function LinkPage({ ctx }: { ctx: Ctx }) {
    const [token, setToken] = useState(ctx.settings?.token ?? '');
    const [path, setPath] = useState(ctx.settings?.savedVariables ?? '');
    const [show, setShow] = useState(false);
    const [me, setMe] = useState<Me | null>(null);
    const [checking, setChecking] = useState(false);

    useEffect(() => {
        setToken(ctx.settings?.token ?? '');
        setPath(ctx.settings?.savedVariables ?? '');
    }, [ctx.settings?.token, ctx.settings?.savedVariables]);

    const dirty = token !== (ctx.settings?.token ?? '') || path !== (ctx.settings?.savedVariables ?? '');

    const test = async () => {
        setChecking(true);
        setMe(null);
        try {
            setMe(await invoke<Me>('test_connection', { token }));
        } catch (e) {
            ctx.notify(String(e), false);
        } finally {
            setChecking(false);
        }
    };

    const save = async () => {
        if (await ctx.save({ token, savedVariables: path })) ctx.notify('Liaison enregistrée. Je veille.');
    };

    return (
        <div className="page">
            <header className="page-head">
                <p className="eyebrow">Liaison</p>
                <h1>Ton compte et ton jeu</h1>
                <p className="lead">Un clic pour ton compte ; ton jeu, je le trouve tout seul.</p>
            </header>

            <section className="panel">
                <h2>
                    <span className="num">1</span> Ton compte
                </h2>
                {ctx.settings?.token ? (
                    <p className="good">
                        <Icon name="check" size={16} /> Application liée à ton compte du site.
                    </p>
                ) : (
                    <p className="dim">Je t’ouvre le site : tu autorises cet ordinateur, et c’est fait.</p>
                )}
                <div className="link-actions">
                    <Pairing ctx={ctx} primary={!ctx.settings?.token} />
                </div>
                <details>
                    <summary>Lier à la main, avec un jeton</summary>
                    <p className="dim small">
                        Crée-le dans ton profil sur le site (Réglages, « Lier à la main, avec un jeton »), puis colle-le ici.{' '}
                        <button type="button" className="link" onClick={() => ctx.open(`${ctx.site}/profil#reglages`)}>
                            Ouvrir mon profil
                        </button>
                    </p>
                    <div className="field">
                        <input
                            type={show ? 'text' : 'password'}
                            value={token}
                            onChange={(e) => setToken(e.target.value)}
                            placeholder="symp_…"
                            spellCheck={false}
                            aria-label="Jeton personnel"
                        />
                        <button type="button" className="icon-btn" onClick={() => setShow(!show)} title={show ? 'Masquer' : 'Afficher'}>
                            <Icon name="eye" size={18} />
                        </button>
                        <button type="button" className="btn ghost" onClick={test} disabled={checking || !token}>
                            {checking ? 'Vérification…' : 'Tester'}
                        </button>
                    </div>
                    {me && (
                        <p className="good">
                            <Icon name="check" size={16} /> Connecté en tant que <strong>{me.name}</strong>
                            {me.characters.length > 0 && <> · {me.characters.map((c) => `${c.name} (${c.level})`).join(', ')}</>}
                        </p>
                    )}
                </details>
            </section>

            <section className="panel">
                <h2>
                    <span className="num">2</span> Fichier de l’addon
                </h2>
                <p className="dim">
                    L’addon Symposium y écrit ton personnage quand tu quittes le jeu ou fais /reload. Pas encore d’addon ? Je l’installe dès que ton
                    compte est lié.
                </p>
                {ctx.candidates.length > 0 ? (
                    <div className="choices">
                        {ctx.candidates.map((c) => (
                            <label key={c.path} className={`choice${path === c.path ? ' chosen' : ''}`}>
                                <input type="radio" name="sv" checked={path === c.path} onChange={() => setPath(c.path)} />
                                <Icon name="folder" size={20} />
                                <span>
                                    <strong>{FLAVORS[c.flavor] ?? c.flavor}</strong> · compte {c.account}
                                    {!c.ready && <em className="dim"> · l’addon n’y a encore rien écrit</em>}
                                    <small>{c.path}</small>
                                </span>
                            </label>
                        ))}
                    </div>
                ) : (
                    <p className="warn">
                        Je ne trouve pas ton jeu. Lance-le une fois et connecte-toi à un personnage, puis « Rechercher à nouveau ». Installé ailleurs ?
                        Indique le dossier World of Warcraft ci-dessous.
                    </p>
                )}
                <details>
                    <summary>Indiquer le chemin à la main</summary>
                    <input
                        type="text"
                        value={path}
                        onChange={(e) => setPath(e.target.value)}
                        placeholder={
                            mac
                                ? '/Applications/World of Warcraft/_classic_beta_/WTF/Account/…/SavedVariables/Symposium.lua'
                                : 'C:\\Program Files (x86)\\World of Warcraft\\_classic_beta_\\WTF\\Account\\…\\SavedVariables\\Symposium.lua'
                        }
                        spellCheck={false}
                        aria-label="Chemin du fichier Symposium.lua"
                    />
                </details>
                <button type="button" className="btn ghost small" onClick={ctx.detect}>
                    <Icon name="refresh" size={16} /> Rechercher à nouveau
                </button>
            </section>

            <div className={`savebar${dirty ? ' visible' : ''}`}>
                <span>Modifications non enregistrées</span>
                <button type="button" className="btn primary" onClick={save} disabled={!dirty}>
                    Enregistrer
                </button>
            </div>
        </div>
    );
}
