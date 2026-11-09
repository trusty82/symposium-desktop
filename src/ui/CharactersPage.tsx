import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import type { Ctx } from '../App';
import { ago, type MeCharacter } from '../types';
import Icon from './Icon';

/** Les personnages du compte sur le site, et le choix du préféré. */
export default function CharactersPage({ ctx }: { ctx: Ctx }) {
    const [characters, setCharacters] = useState<MeCharacter[] | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [saving, setSaving] = useState<number | null>(null);

    const load = useCallback(() => {
        setError(null);
        invoke<MeCharacter[]>('my_characters')
            .then(setCharacters)
            .catch((e) => setError(String(e)));
    }, []);
    useEffect(load, [load]);

    const choose = async (c: MeCharacter) => {
        setSaving(c.id);
        try {
            setCharacters(await invoke<MeCharacter[]>('set_main_character', { id: c.id }));
            ctx.notify(`${c.name} est ton personnage préféré.`);
        } catch (e) {
            ctx.notify(String(e), false);
        } finally {
            setSaving(null);
        }
    };

    // Site plus ancien : pas de numéro de personnage, donc pas de choix possible ici.
    const canChoose = characters?.every((c) => c.id > 0) ?? false;

    return (
        <div className="page">
            <header className="page-head">
                <p className="eyebrow">Personnages</p>
                <h1>Ta bande</h1>
                <p className="lead">Tes personnages sur le site. Choisis ton préféré : il te représente en tête de ton profil et de la liste de la guilde.</p>
            </header>

            {error ? (
                <section className="panel">
                    <p className="bad">
                        <Icon name="alert" size={16} /> {error}
                    </p>
                    <button type="button" className="btn ghost" onClick={load}>
                        <Icon name="refresh" size={18} /> Réessayer
                    </button>
                </section>
            ) : characters === null ? (
                <section className="panel">
                    <p className="dim">Je demande la liste au site…</p>
                </section>
            ) : characters.length === 0 ? (
                <section className="panel">
                    <p>Aucun personnage sur le site pour l’instant.</p>
                    <p className="dim">Quitte le jeu une fois avec l’addon installé : ta fiche partira toute seule. Tu peux aussi lier Battle.net sur ton profil.</p>
                    <button type="button" className="btn ghost" onClick={() => ctx.open(`${ctx.site}/profil`)}>
                        <Icon name="external" size={18} /> Ouvrir mon profil
                    </button>
                </section>
            ) : (
                <ul className="characters">
                    {characters.map((c) => (
                        <li key={c.id || c.name} className={`panel character${c.main ? ' character-main' : ''}`}>
                            <span className="character-bar" style={{ background: c.classColor ?? 'var(--line-strong)' }} aria-hidden="true" />
                            <div className="character-body">
                                <strong className="character-name" style={c.classColor ? { color: c.classColor } : undefined}>
                                    {c.name}
                                </strong>
                                <small>
                                    Niveau {c.level} · {c.className ?? c.class}
                                    {c.raceName && ` ${c.raceName.toLowerCase()}`}
                                    {c.spec && ` · ${c.spec}`}
                                    {c.itemLevel ? ` · niv. d’objet ${Math.round(c.itemLevel)}` : ''}
                                </small>
                                {c.exportedAt && <small className="dim">Fiche envoyée {ago(Date.parse(c.exportedAt) / 1000)}</small>}
                            </div>
                            <div className="character-actions">
                                {c.main && c.mainChosen ? (
                                    <span className="badge">
                                        <Icon name="check" size={14} /> Préféré
                                    </span>
                                ) : (
                                    canChoose && (
                                        <button type="button" className="btn small" onClick={() => choose(c)} disabled={saving !== null}>
                                            {saving === c.id ? 'Un instant…' : 'Choisir comme préféré'}
                                        </button>
                                    )
                                )}
                                {c.url && (
                                    <button type="button" className="link" onClick={() => ctx.open(c.url!)}>
                                        Voir sur le site
                                    </button>
                                )}
                            </div>
                        </li>
                    ))}
                </ul>
            )}
            {characters && characters.length > 0 && !characters.some((c) => c.mainChosen) && canChoose && (
                <p className="dim">Pas encore de préféré choisi : le site prend ton personnage le plus avancé, en tête de la liste.</p>
            )}
        </div>
    );
}
