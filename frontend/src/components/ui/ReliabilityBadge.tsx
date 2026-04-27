import type { ReliabilityLevel } from '@/src/types/api.types'
import { clsx } from 'clsx'
import { RELIABILITY_BADGE } from '@/src/types/api.types'

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