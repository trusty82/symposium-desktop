import Icon from './Icon';

export interface Toast {
    id: number;
    message: string;
    ok: boolean;
}

export default function Toasts({ toasts }: { toasts: Toast[] }) {
    return (
        <div className="toasts" role="status" aria-live="polite">
            {toasts.map((t) => (
                <div key={t.id} className={`toast ${t.ok ? 'ok' : 'bad'}`}>
                    <Icon name={t.ok ? 'check' : 'alert'} size={18} />
                    <span>{t.message}</span>
                </div>
            ))}
        </div>
    );
}
