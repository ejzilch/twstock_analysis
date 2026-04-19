import { type ReactNode } from 'react'
import { clsx } from 'clsx'

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