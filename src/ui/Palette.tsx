import { useEffect, useMemo, useRef, useState } from 'react';
import Icon, { type IconName } from './Icon';

export interface Command {
    id: string;
    label: string;
    icon: IconName;
    run: () => unknown;
    disabled?: boolean;
}

const normalize = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');

/** Palette de commandes (⌘K / Ctrl+K) : tout l'essentiel au clavier. */
export default function Palette({ commands, onClose }: { commands: Command[]; onClose: () => void }) {
    const [query, setQuery] = useState('');
    const [index, setIndex] = useState(0);
    const input = useRef<HTMLInputElement>(null);

    const shown = useMemo(() => {
        const words = normalize(query).split(/\s+/).filter(Boolean);
        return commands.filter((c) => !c.disabled && words.every((w) => normalize(c.label).includes(w)));
    }, [commands, query]);

    useEffect(() => input.current?.focus(), []);
    useEffect(() => setIndex(0), [query]);

    const run = (c: Command | undefined) => {
        if (!c) return;
        onClose();
        void c.run();
    };

    return (
        <div className="palette-backdrop" onMouseDown={onClose}>
            <div
                className="palette"
                role="dialog"
                aria-modal="true"
                aria-label="Palette de commandes"
                onMouseDown={(e) => e.stopPropagation()}
                onKeyDown={(e) => {
                    if (e.key === 'Escape') onClose();
                    if (e.key === 'ArrowDown') {
                        e.preventDefault();
                        setIndex((i) => Math.min(i + 1, shown.length - 1));
                    }
                    if (e.key === 'ArrowUp') {
                        e.preventDefault();
                        setIndex((i) => Math.max(i - 1, 0));
                    }
                    if (e.key === 'Enter') run(shown[index]);
                }}
            >
                <div className="palette-input">
                    <Icon name="search" size={18} />
                    <input
                        ref={input}
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        placeholder="Que veux-tu faire ?"
                        aria-label="Rechercher une commande"
                        aria-controls="palette-list"
                    />
                    <kbd>Échap</kbd>
                </div>
                <ul id="palette-list" role="listbox">
                    {shown.map((c, i) => (
                        <li key={c.id} role="option" aria-selected={i === index}>
                            <button type="button" className={i === index ? 'selected' : ''} onMouseEnter={() => setIndex(i)} onClick={() => run(c)}>
                                <Icon name={c.icon} size={18} />
                                {c.label}
                            </button>
                        </li>
                    ))}
                    {shown.length === 0 && <li className="palette-empty">Aucune commande.</li>}
                </ul>
            </div>
        </div>
    );
}
