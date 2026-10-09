/** Interrupteur accessible (case à cocher stylée). */
export default function Switch({
    checked,
    onChange,
    label,
    hint,
    disabled,
}: {
    checked: boolean;
    onChange: () => void;
    label: string;
    hint?: string;
    disabled?: boolean;
}) {
    return (
        <label className={`switch${disabled ? ' disabled' : ''}`}>
            <input type="checkbox" role="switch" checked={checked} onChange={onChange} disabled={disabled} />
            <span className="switch-track" aria-hidden="true">
                <span className="switch-thumb" />
            </span>
            <span className="switch-text">
                <span>{label}</span>
                {hint && <small>{hint}</small>}
            </span>
        </label>
    );
}
