/** Icônes au trait, dessinées pour l'application (pas de bibliothèque). */
const PATHS = {
    home: 'M3 11.5 12 4l9 7.5M5.5 10v9.5h4.5v-6h4v6h4.5V10',
    link: 'M10 14a4.5 4.5 0 0 0 6.4 0l3-3a4.5 4.5 0 0 0-6.4-6.4l-1 1M14 10a4.5 4.5 0 0 0-6.4 0l-3 3a4.5 4.5 0 0 0 6.4 6.4l1-1',
    addon: 'M9 3.5h6v3a2 2 0 1 0 0 4v0h3.5V15a2 2 0 1 1 0 4v1.5H5.5V17a2 2 0 1 0 0-4V9.5H9a2 2 0 1 1 0-4z',
    combat: 'M5 4l9.5 9.5M4 5l1-1 3 .5 9 9-1.5 1.5-9-9zM19 4l-6 6M20 5l-1-1-3 .5M14.5 15.5 17 18l1.5-1.5L21 19l-2 2-2.5-2.5L15 20l-2.5-2.5M9.5 15.5 7 18l-1.5-1.5L3 19l2 2 2.5-2.5L9 20l2.5-2.5',
    settings: 'M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 13a7.6 7.6 0 0 0 0-2l2-1.6-2-3.4-2.4 1a7.4 7.4 0 0 0-1.7-1L15 3.5h-4l-.4 2.5a7.4 7.4 0 0 0-1.7 1l-2.4-1-2 3.4L6.6 11a7.6 7.6 0 0 0 0 2l-2 1.6 2 3.4 2.4-1a7.4 7.4 0 0 0 1.7 1l.4 2.5h4l.4-2.5a7.4 7.4 0 0 0 1.7-1l2.4 1 2-3.4z',
    search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4',
    send: 'M4 12 20 4l-4 16-4-7zM12 13l8-9',
    refresh: 'M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6',
    external: 'M14 4h6v6M20 4l-9 9M18 14v5H5V6h5',
    check: 'M5 12.5 10 17l9-10',
    alert: 'M12 4 2.5 20h19zM12 10v4M12 17v.5',
    eye: 'M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12zM12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z',
    folder: 'M3.5 6.5h6l2 2h9v10h-17z',
    spark: 'M12 3v4M12 17v4M3 12h4M17 12h4M6 6l2.5 2.5M15.5 15.5 18 18M18 6l-2.5 2.5M8.5 15.5 6 18',
    arrow: 'M5 12h14M13 6l6 6-6 6',
} as const;

export type IconName = keyof typeof PATHS;

export default function Icon({ name, size = 20, className }: { name: IconName; size?: number; className?: string }) {
    return (
        <svg
            className={className}
            width={size}
            height={size}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.6}
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
        >
            <path d={PATHS[name]} />
        </svg>
    );
}
