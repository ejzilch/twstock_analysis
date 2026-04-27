'use client'
import type { SymbolItem } from '@/src/types/api.types'
import { useAppStore } from '@/src/store/useAppStore'
import { useSymbols } from '@/src/hooks'

export function SymbolSelector() {
    const { data, isLoading } = useSymbols()
    const selected = useAppStore((s) => s.selectedSymbol)
    const setSelected = useAppStore((s) => s.setSelectedSymbol)

    if (isLoading) return (
        <div className="h-9 w-40 bg-surface-card border border-surface-border rounded-lg animate-pulse" />
    )

    const options = (data?.symbols ?? []).map((s: SymbolItem) => ({
        value: s.symbol,
        label: `${s.symbol} ${s.name}`,
    }))

    return (
        <select
            value={selected}
            onChange={(e) => setSelected(e.target.value)}
            className="bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-brand-500/50 min-w-[160px]"
        >
            {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
        </select>
    )
}