import { clsx } from 'clsx'
import type { Interval } from '@/src/types/api.types'
import { useAppStore } from '@/src/store/useAppStore'

const INTERVALS: { value: Interval; label: string }[] = [
    { value: '1m', label: '1 分' },
    { value: '5m', label: '5 分' },
    { value: '15m', label: '15 分' },
    { value: '1h', label: '1 時' },
    { value: '4h', label: '4 時' },
    { value: '1d', label: '日線' },
]

export function IntervalSelector() {
    const selected = useAppStore((s) => s.selectedInterval)
    const setSelected = useAppStore((s) => s.setSelectedInterval)

    return (
        <div className="flex items-center gap-1 bg-surface-card border border-surface-border rounded-lg p-1">
            {INTERVALS.map((i) => (
                <button
                    key={i.value}
                    onClick={() => setSelected(i.value)}
                    className={clsx(
                        'px-3 py-1 rounded-md text-xs font-medium transition-all',
                        selected === i.value
                            ? 'bg-brand-600 text-white shadow-sm'
                            : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover',
                    )}
                >
                    {i.label}
                </button>
            ))}
        </div>
    )
}