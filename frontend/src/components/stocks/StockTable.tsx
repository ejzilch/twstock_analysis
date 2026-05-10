'use client'
import { useState, useEffect } from 'react'
import { clsx } from 'clsx'
import type { SymbolItem } from '@/src/types/api.types'
import { Card, Input } from '@/src/components/ui'
import { StockRow } from './StockRow'

interface StockTableProps {
    symbols: SymbolItem[]
    /** 由 page 層提升管理，控制 useSymbols fetch 行為 */
    includeInactive: boolean
    onToggleInactive: () => void
}

const PAGE_SIZE = 50

type ExchangeFilter = 'ALL' | 'TWSE' | 'TPEX'

interface StockTableControlsProps {
    search: string
    onSearch: (v: string) => void
    exchange: ExchangeFilter
    onExchange: (v: ExchangeFilter) => void
    includeInactive: boolean
    onToggleInactive: () => void
    filteredCount: number
}

function StockTableControls({
    search, onSearch,
    exchange, onExchange,
    includeInactive, onToggleInactive,
    filteredCount,
}: StockTableControlsProps) {
    return (
        <div className="flex flex-wrap items-center gap-3">
            <Input
                value={search}
                onChange={onSearch}
                placeholder="搜尋股票代號或名稱..."
                className="max-w-xs"
            />

            {/* 交易所 filter */}
            <div className="flex items-center gap-1 bg-surface-card border border-surface-border rounded-lg p-1">
                {(['ALL', 'TWSE', 'TPEX'] as const).map((ex) => (
                    <button
                        key={ex}
                        onClick={() => onExchange(ex)}
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

            {/* 含下市 toggle */}
            <button
                onClick={onToggleInactive}
                className={clsx(
                    'flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs border transition-all',
                    includeInactive
                        ? 'bg-slate-700/60 border-slate-500/40 text-slate-300'
                        : 'bg-surface-card border-surface-border text-slate-500 hover:text-slate-300',
                )}
            >
                <span className={clsx(
                    'w-3.5 h-3.5 rounded-sm border flex items-center justify-center transition-colors',
                    includeInactive ? 'bg-slate-500 border-slate-400' : 'border-slate-600',
                )}>
                    {includeInactive && <span className="text-white text-[10px] leading-none">✓</span>}
                </span>
                含下市股票
            </button>

            <span className="text-xs text-slate-500 ml-auto">{filteredCount} 檔</span>
        </div>
    )
}

function Pagination({
    currentPage,
    totalPages,
    onPage,
}: {
    currentPage: number
    totalPages: number
    onPage: (p: number) => void
}) {
    if (totalPages <= 1) return null

    // 顯示最多 5 個頁碼，以 currentPage 為中心
    const getPageNumbers = () => {
        const delta = 2
        const range: number[] = []
        for (
            let i = Math.max(1, currentPage - delta);
            i <= Math.min(totalPages, currentPage + delta);
            i++
        ) {
            range.push(i)
        }
        return range
    }

    const pages = getPageNumbers()

    return (
        <div className="flex items-center justify-center gap-1.5 px-5 py-3 border-t border-surface-border">
            <button
                onClick={() => onPage(currentPage - 1)}
                disabled={currentPage === 1}
                className="px-2.5 py-1 rounded-md text-xs text-slate-400 hover:text-slate-200 hover:bg-surface-hover disabled:opacity-30 disabled:cursor-not-allowed transition-all"
            >
                ←
            </button>

            {pages[0] > 1 && (
                <>
                    <button
                        onClick={() => onPage(1)}
                        className="px-2.5 py-1 rounded-md text-xs text-slate-400 hover:text-slate-200 hover:bg-surface-hover transition-all"
                    >
                        1
                    </button>
                    {pages[0] > 2 && (
                        <span className="px-1 text-xs text-slate-600">…</span>
                    )}
                </>
            )}

            {pages.map((p) => (
                <button
                    key={p}
                    onClick={() => onPage(p)}
                    className={clsx(
                        'px-2.5 py-1 rounded-md text-xs font-medium transition-all',
                        p === currentPage
                            ? 'bg-brand-600 text-white'
                            : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover',
                    )}
                >
                    {p}
                </button>
            ))}

            {pages[pages.length - 1] < totalPages && (
                <>
                    {pages[pages.length - 1] < totalPages - 1 && (
                        <span className="px-1 text-xs text-slate-600">…</span>
                    )}
                    <button
                        onClick={() => onPage(totalPages)}
                        className="px-2.5 py-1 rounded-md text-xs text-slate-400 hover:text-slate-200 hover:bg-surface-hover transition-all"
                    >
                        {totalPages}
                    </button>
                </>
            )}

            <button
                onClick={() => onPage(currentPage + 1)}
                disabled={currentPage === totalPages}
                className="px-2.5 py-1 rounded-md text-xs text-slate-400 hover:text-slate-200 hover:bg-surface-hover disabled:opacity-30 disabled:cursor-not-allowed transition-all"
            >
                →
            </button>

            <span className="ml-2 text-xs text-slate-600">
                第 {currentPage} / {totalPages} 頁
            </span>
        </div>
    )
}

export function StockTable({ symbols, includeInactive, onToggleInactive }: StockTableProps) {
    const [search, setSearch] = useState('')
    const [exchange, setExchange] = useState<ExchangeFilter>('ALL')
    const [currentPage, setCurrentPage] = useState(1)

    // filter 變動時 reset 頁碼
    useEffect(() => {
        setCurrentPage(1)
    }, [search, exchange, includeInactive])

    // includeInactive 來自 props，filter 只做視覺過濾（symbols 已含 inactive）
    const filtered = symbols.filter((s) => {
        const matchSearch = s.symbol.includes(search) || s.name.includes(search)
        const matchExchange = exchange === 'ALL' || s.exchange === exchange
        const matchActive = includeInactive ? true : s.is_active
        return matchSearch && matchExchange && matchActive
    })

    const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE))
    const paginated = filtered.slice(
        (currentPage - 1) * PAGE_SIZE,
        currentPage * PAGE_SIZE,
    )

    return (
        <div className="flex flex-col gap-4">
            <StockTableControls
                search={search}
                onSearch={setSearch}
                exchange={exchange}
                onExchange={setExchange}
                includeInactive={includeInactive}
                onToggleInactive={onToggleInactive}
                filteredCount={filtered.length}
            />

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
                            {paginated.map((s) => (
                                <StockRow key={s.symbol} symbol={s} />
                            ))}
                        </tbody>
                    </table>

                    {filtered.length === 0 && (
                        <div className="py-12 text-center text-sm text-slate-500">
                            找不到符合條件的股票
                        </div>
                    )}
                </div>

                <Pagination
                    currentPage={currentPage}
                    totalPages={totalPages}
                    onPage={setCurrentPage}
                />
            </Card>
        </div>
    )
}