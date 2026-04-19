import { InputHTMLAttributes } from 'react'
import { clsx } from 'clsx'

interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange'> {
    label?: string
    value: string
    onChange: (v: string) => void
    placeholder?: string
    type?: string
    error?: string
    className?: string
}

export function Input({ label, value, onChange, placeholder, type = 'text', error, className, ...props }: InputProps) {
    return (
        <div className={clsx('flex flex-col gap-1.5', className)}>
            {label && <label className="text-xs font-medium text-slate-400 uppercase tracking-wider">{label}</label>}
            <input
                type={type}
                value={value}
                onChange={(e) => onChange(e.target.value)}
                placeholder={placeholder}
                className={clsx(
                    'bg-surface border rounded-lg px-3 py-2 text-sm text-slate-200 placeholder-slate-600',
                    'focus:outline-none focus:ring-2 focus:ring-brand-500/50 transition-all',
                    error ? 'border-red-500/50' : 'border-surface-border',
                )}
                {...props}
                onClick={(e) => {
                    if (type === 'date' && 'showPicker' in HTMLInputElement.prototype) {
                        try {
                            e.currentTarget.showPicker()
                        } catch (error) {
                            // ignore
                        }
                    }
                    if (props.onClick) props.onClick(e)
                }}
            />
            {error && <p className="text-xs text-red-400">{error}</p>}
        </div>
    )
}