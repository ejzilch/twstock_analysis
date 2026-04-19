import { clsx } from 'clsx'

interface SelectProps<T extends string> {
    value: T
    onChange: (v: T) => void
    options: { value: T; label: string }[]
    label?: string
    className?: string
}

export function Select<T extends string>({ value, onChange, options, label, className }: SelectProps<T>) {
    return (
        <div className={clsx('flex flex-col gap-1.5', className)}>
            {label && <label className="text-xs font-medium text-slate-400 uppercase tracking-wider">{label}</label>}
            <select
                value={value}
                onChange={(e) => onChange(e.target.value as T)}
                className="bg-surface border border-surface-border rounded-lg px-3 py-2 text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
            >
                {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
        </div>
    )
}