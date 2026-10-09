import logo from '../assets/logo.png';
import { type Page } from '../types';
import Icon, { type IconName } from './Icon';

const ITEMS: { page: Page; label: string; icon: IconName }[] = [
    { page: 'home', label: 'Accueil', icon: 'home' },
    { page: 'link', label: 'Liaison', icon: 'link' },
    { page: 'addon', label: 'Addon', icon: 'addon' },
    { page: 'combat', label: 'Combats', icon: 'combat' },
    { page: 'settings', label: 'Réglages', icon: 'settings' },
];

const mac = navigator.userAgent.includes('Mac');

/** Barre de navigation verticale, à gauche. */
export default function Rail({
    page,
    go,
    ready,
    addonAlert,
    onPalette,
}: {
    page: Page;
    go: (p: Page) => void;
    ready: boolean;
    addonAlert: boolean;
    onPalette: () => void;
}) {
    return (
        <nav className="rail" aria-label="Navigation">
            <img src={logo} alt="Symposium" className="rail-logo" />
            <ul>
                {ITEMS.map((item) => (
                    <li key={item.page}>
                        <button
                            type="button"
                            className={`rail-item${page === item.page ? ' active' : ''}`}
                            aria-current={page === item.page ? 'page' : undefined}
                            onClick={() => go(item.page)}
                            title={item.label}
                        >
                            <Icon name={item.icon} size={22} />
                            <span>{item.label}</span>
                            {item.page === 'link' && !ready && <i className="pip warn" aria-label="à configurer" />}
                            {item.page === 'addon' && addonAlert && <i className="pip gold" aria-label="mise à jour disponible" />}
                        </button>
                    </li>
                ))}
            </ul>
            <button type="button" className="rail-palette" onClick={onPalette} title="Palette de commandes">
                <Icon name="search" size={18} />
                <kbd>{mac ? '⌘K' : 'Ctrl K'}</kbd>
            </button>
        </nav>
    );
}
