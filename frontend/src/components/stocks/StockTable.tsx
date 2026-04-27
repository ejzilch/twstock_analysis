'use client'
import { useState } from 'react'
import { clsx } from 'clsx'
import type { SymbolItem } from '@/src/types/api.types'
import { Card, Input } from '@/src/components/ui'
import { StockRow } from './StockRow'

interface StockTableProps { symbols: SymbolItem[] }

export function StockTable({ symbols }: StockTableProps) {
    const [search, setSearch] = useState('')
    const [exchange, setExchange] = useState<'ALL' | 'TWSE' | 'TPEX'>('ALL')

    const filtered = symbols.filter((s) => {
        const matchSearch = s.symbol.includes(search) || s.name.includes(search)
        const matchExchange = exchange === 'ALL' || s.exchange === exchange
        return matchSearch && matchExchange
    })

    return (
        <div className="flex flex-col gap-4">
            <div className="flex items-center gap-3">
                <Input
                    value={search}
                    onChange={setSearch}
                    placeholder="搜尋股票代號或名稱..."
                    className="max-w-xs"
                />
                <div className="flex items-center gap-1 bg-surface-card border border-surface-border rounded-lg p-1">
                    {(['ALL', 'TWSE', 'TPEX'] as const).map((ex) => (
                        <button
                            key={ex}
                            onClick={() => setExchange(ex)}
                            className={clsx(
                                'px-3 py-1 rounded-md text-xs font-medium transition-all',
                                exchange === ex
                                    ? 'bg-brand-600 text-white'
                                    : 'text-slate-400 hover:text-slate-200',
                            )}
                        >
                            {ex === 'ALL' ? '全部' : ex}
                        </button>
                    ))}
                </div>
                <span className="text-xs text-slate-500 ml-auto">{filtered.length} 檔</span>
            </div>

            <Card padding={false}>
                <div className="overflow-x-auto">
                    <table className="w-full text-sm">
                        <thead>
                            <tr className="border-b border-surface-border">
                                <th className="px-5 py-3 text-left text-xs font-medium text-slate-400 uppercase tracking-wider">代號</th>
                                <th className="px-5 py-3 text-left text-xs font-medium text-slate-400 uppercase tracking-wider">名稱</th>
                                <th className="px-5 py-3 text-left text-xs font-medium text-slate-400 uppercase tracking-wider">交易所</th>
                                <th className="px-5 py-3 text-left text-xs font-medium text-slate-400 uppercase tracking-wider">資料來源</th>
                                <th className="px-5 py-3 text-left text-xs font-medium text-slate-400 uppercase tracking-wider">狀態</th>
                            </tr>
                        </thead>
                        <tbody className="divide-y divide-surface-border">
                            {filtered.map((s) => (
                                <StockRow key={s.symbol} symbol={s} />
                            ))}
                        </tbody>
                    </table>
                    {filtered.length === 0 && (
                        <div className="py-12 text-center text-sm text-slate-500">找不到符合條件的股票</div>
                    )}
                </div>
            </Card>
        </div>
    )
}