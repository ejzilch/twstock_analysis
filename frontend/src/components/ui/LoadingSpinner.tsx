import { clsx } from 'clsx'

export function LoadingSpinner({ size = 'md', label }: { size?: 'sm' | 'md' | 'lg'; label?: string }) {
    const sizes = { sm: 'w-4 h-4', md: 'w-6 h-6', lg: 'w-10 h-10' }
    return (
        <div className="flex flex-col items-center justify-center gap-3">
            <div className={clsx('border-2 border-brand-600/30 border-t-brand-500 rounded-full animate-spin', sizes[size])} />
            {label && <p className="text-sm text-slate-500">{label}</p>}
        </div>
    )
}