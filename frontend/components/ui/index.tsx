/**
 * Atomic UI components — no API or store dependencies.
 * Pure props-driven, reusable building blocks.
 */
'use client'
import { type ReactNode, useEffect, useState, InputHTMLAttributes } from 'react'
import { clsx } from 'clsx'
import type { ReliabilityLevel, ErrorCode } from '@/types/api.generated'
import { RELIABILITY_BADGE } from '@/types/app'
import { resolveErrorAction } from '@/lib/error-handler'

// ── Badge ─────────────────────────────────────────────────────────────────────

interface BadgeProps { reliability: ReliabilityLevel }

export function ReliabilityBadge({ reliability }: BadgeProps) {
    const cfg = RELIABILITY_BADGE[reliability]
    return (
        <span className={clsx('inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium', cfg.bg, cfg.text)}>
            <span className="w-1.5 h-1.5 rounded-full bg-current opacity-70" />
            {cfg.label}
        </span>
    )
}

// ── Button ────────────────────────────────────────────────────────────────────

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

// ── Card ──────────────────────────────────────────────────────────────────────

interface CardProps {
    children: ReactNode
    className?: string
    padding?: boolean
}

export function Card({ children, className, padding = true }: CardProps) {
    return (
        <div className={clsx(
            'bg-surface-card border border-surface-border rounded-xl',
            padding && 'p-5',
            className,
        )}>
            {children}
        </div>
    )
}

// ── Input ─────────────────────────────────────────────────────────────────────

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

// ── LoadingSpinner ────────────────────────────────────────────────────────────

export function LoadingSpinner({ size = 'md', label }: { size?: 'sm' | 'md' | 'lg'; label?: string }) {
    const sizes = { sm: 'w-4 h-4', md: 'w-6 h-6', lg: 'w-10 h-10' }
    return (
        <div className="flex flex-col items-center justify-center gap-3">
            <div className={clsx('border-2 border-brand-600/30 border-t-brand-500 rounded-full animate-spin', sizes[size])} />
            {label && <p className="text-sm text-slate-500">{label}</p>}
        </div>
    )
}

// ── Toast ─────────────────────────────────────────────────────────────────────

interface ToastMessage {
    id: string
    message: string
    variant: 'info' | 'warning' | 'error'
    onRetry?: () => void
}

interface ToastProps { error: unknown; onRetry?: () => void; onRedirect?: (path: string) => void }

export function ErrorToast({ error, onRetry, onRedirect }: ToastProps) {
    const [visible, setVisible] = useState(true)
    const action = resolveErrorAction(error)

    useEffect(() => {
        if (action.variant === 'silent') return
        const t = setTimeout(() => setVisible(false), 5000)
        return () => clearTimeout(t)
    }, [action.variant])

    if (action.variant === 'silent' || !visible) return null

    const colors = {
        info: 'border-brand-500/30 bg-brand-900/40 text-brand-300',
        warning: 'border-amber-500/30 bg-amber-900/30 text-amber-300',
        error: 'border-red-500/30 bg-red-900/30 text-red-300',
    }

    return (
        <div className={clsx(
            'fixed bottom-5 right-5 z-50 max-w-sm w-full border rounded-xl px-4 py-3 shadow-2xl animate-slide-up',
            colors[action.variant],
        )}>
            <div className="flex items-start gap-3">
                <span className="mt-0.5 text-base">
                    {action.variant === 'warning' ? '⚠️' : action.variant === 'error' ? '❌' : 'ℹ️'}
                </span>
                <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium">{action.message}</p>
                    {action.showRetry && onRetry && (
                        <button onClick={onRetry} className="mt-1.5 text-xs underline opacity-80 hover:opacity-100">
                            重試
                        </button>
                    )}
                    {action.redirect && onRedirect && (
                        <button onClick={() => onRedirect(action.redirect!)} className="mt-1.5 text-xs underline opacity-80 hover:opacity-100">
                            前往設定
                        </button>
                    )}
                </div>
                <button onClick={() => setVisible(false)} className="text-current opacity-50 hover:opacity-100 text-lg leading-none">×</button>
            </div>
        </div>
    )
}

// ── Select ────────────────────────────────────────────────────────────────────

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