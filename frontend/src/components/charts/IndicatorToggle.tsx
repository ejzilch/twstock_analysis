'use client'
import { clsx } from 'clsx'
import { INDICATOR_COLORS } from '@/src/constants/chartColors'

const TOGGLES = [
    { key: 'ma5', label: 'MA5', color: INDICATOR_COLORS.ma5 },
    { key: 'ma20', label: 'MA20', color: INDICATOR_COLORS.ma20 },
    { key: 'ma50', label: 'MA50', color: INDICATOR_COLORS.ma50 },
    { key: 'bollinger', label: 'BOLL', color: INDICATOR_COLORS.bollMid },
] as const

type IndicatorKey = typeof TOGGLES[number]['key']

interface IndicatorToggleProps {
    visible: Set<string>
    onChange: (next: Set<string>) => void
}

export function IndicatorToggle({ visible, onChange }: IndicatorToggleProps) {
    function toggle(key: string) {
        const next = new Set(visible)
        next.has(key) ? next.delete(key) : next.add(key)
        onChange(next)
    }

    return (
        <div className="flex items-center gap-1.5 flex-wrap">
            {TOGGLES.map(({ key, label, color }) => {
                const active = visible.has(key)
                return (
                    <button
                        key={key}
                        onClick={() => toggle(key)}
                        className={clsx(
                            'flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-all border',
                            active
                                ? 'bg-surface-hover border-surface-border text-slate-200'
                                : 'border-transparent text-slate-600 hover:text-slate-400',
                        )}
                    >
                        <span
                            className="w-2.5 h-0.5 rounded-full inline-block"
                            style={{ backgroundColor: active ? color : '#475569' }}
                        />
                        {label}
                    </button>
                )
            })}
        </div>
    )
}