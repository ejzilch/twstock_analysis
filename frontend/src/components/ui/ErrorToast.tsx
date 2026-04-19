'use client'
import { useEffect, useState } from 'react'
import { clsx } from 'clsx'
import { resolveErrorAction } from '@/src/lib/error-handler'

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