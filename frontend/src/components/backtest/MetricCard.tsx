import { clsx } from 'clsx'
import { useAppStore } from '@/src/store/useAppStore'

interface MetricCardProps { label: string; value: string; sub?: string; positive?: boolean }

export function MetricCard({ label, value, sub, positive }: MetricCardProps) {
    const colorMode = useAppStore((s) => s.colorMode)

    const positiveColor = colorMode === 'TW' ? 'text-red-400' : 'text-emerald-400'
    const negativeColor = colorMode === 'TW' ? 'text-emerald-400' : 'text-red-400'

    return (
        <div className="bg-surface border border-surface-border rounded-lg p-4" >
            <div className="text-xs text-slate-500 uppercase tracking-wider mb-1.5" > {label} </div>
            < div className={
                clsx(
                    'text-2xl font-bold font-mono',
                    positive === true ? positiveColor : positive === false ? negativeColor : 'text-slate-200'
                )
            }>
                {value}
            </div>
            {sub && <div className="text-xs text-slate-500 mt-1" > {sub} </div>}
        </div>
    )
}