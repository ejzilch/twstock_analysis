import { type ReactNode } from 'react'
import { clsx } from 'clsx'

interface ButtonProps {
    children: ReactNode
    onClick?: () => void
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
    size?: 'sm' | 'md' | 'lg'
    disabled?: boolean
    loading?: boolean
    className?: string
    type?: 'button' | 'submit' | 'reset'
}

export function Button({
    children, onClick, variant = 'primary', size = 'md',
    disabled, loading, className, type = 'button',
}: ButtonProps) {
    const base = 'inline-flex items-center justify-center gap-2 font-medium rounded-lg transition-all duration-150 focus:outline-none focus:ring-2 focus:ring-brand-500/50 disabled:opacity-50 disabled:cursor-not-allowed'
    const variants = {
        primary: 'bg-brand-600 hover:bg-brand-500 text-white shadow-sm shadow-brand-900/30',
        secondary: 'bg-surface-card border border-surface-border hover:bg-surface-hover text-slate-300',
        ghost: 'hover:bg-surface-hover text-slate-400 hover:text-slate-200',
        danger: 'bg-red-600/20 hover:bg-red-600/30 text-red-400 border border-red-500/30',
    }
    const sizes = { sm: 'px-3 py-1.5 text-xs', md: 'px-4 py-2 text-sm', lg: 'px-5 py-2.5 text-base' }

    return (
        <button
            type={type}
            onClick={onClick}
            disabled={disabled || loading}
            className={clsx(base, variants[variant], sizes[size], className)}
        >
            {loading && <span className="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin" />}
            {children}
        </button>
    )
}