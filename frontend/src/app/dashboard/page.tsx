'use client'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useRouter } from 'next/navigation'
import { clsx } from 'clsx'
import { IconSettings, IconChartLine } from '@tabler/icons-react'

// Hooks 類
import { useCandles, useSignals, useChartSync, useSymbols } from '@/src/hooks'
import { useAppStore } from '@/src/store/useAppStore'

// 組件類 (Charts, Dashboard, UI)
import { CandleChart, IndicatorPane } from '@/src/components/charts'
import { IndicatorToggle } from '@/src/components/charts/IndicatorToggle'
import { SymbolSelector, IntervalSelector, SignalList, PredictionPanel } from '@/src/components/dashboard'
import { LoadingSpinner, ErrorToast, Card } from '@/src/components/ui'

// 工具與型別
import { ApiErrorException } from '@/src/lib/api-client'
import type { CrosshairData } from '@/src/hooks/useChartSync'
import type { DashboardLeftPanelId } from '@/src/types/api.types'

const TIME_RANGES = [
    { label: '1D', value: 1 },
    { label: '5D', value: 5 },
    { label: '1M', value: 30 },
    { label: '3M', value: 90 },
    { label: '6M', value: 180 },
    { label: '1Y', value: 365 },
    { label: '3Y', value: 365 * 3 },
    { label: 'MAX', value: 'max' as const },
]

const MAX_LOOKBACK_DAYS_BY_INTERVAL: Record<string, number> = {
    '1m': 1, '5m': 6, '15m': 20, '1h': 79, '4h': 316, '1d': 1900,
}

const LEFT_LABEL: Record<DashboardLeftPanelId, string> = {
    candles: 'Candles',
    rsi: 'RSI (14)',
    macd: 'MACD (12,26,9)',
    institutionalNetFlow: 'Institutional Net Flow',
}

function isDataSourceError(error: unknown): boolean {
    return error instanceof ApiErrorException &&
        (error.errorCode === 'DATA_SOURCE_INTERRUPTED' || error.errorCode === 'DATA_SOURCE_RATE_LIMITED')
}

function fmt(n: number | null | undefined, d = 2) {
    if (n == null || Number.isNaN(n)) return '--'
    return n.toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d })
}

function reorderLeftPanels(order: DashboardLeftPanelId[], fromId: DashboardLeftPanelId, toId: DashboardLeftPanelId) {
    if (fromId === toId) return order
    const next = [...order]
    const fromIndex = next.indexOf(fromId)
    const toIndex = next.indexOf(toId)
    if (fromIndex < 0 || toIndex < 0) return order
    const [moved] = next.splice(fromIndex, 1)
    next.splice(toIndex, 0, moved)
    return next
}

function Stat({ label, value, accent }: { label: string; value: string; accent?: 'up' | 'down' }) {
    return (
        <div className="min-w-[120px] rounded-lg border border-surface-border bg-surface-card px-3 py-2">
            <div className="text-[11px] uppercase tracking-wider text-slate-500">{label}</div>
            <div className={clsx('mt-1 text-sm font-medium', accent === 'up' && 'text-rose-300', accent === 'down' && 'text-emerald-300')}>
                {value}
            </div>
        </div>
    )
}

function DataLatencyBanner() {
    return (
        <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
            Data source limited. Cached values may be shown.
        </div>
    )
}

export default function DashboardPage() {
    const chartSync = useChartSync()
    const router = useRouter()
    const symbol = useAppStore((s) => s.selectedSymbol)
    const interval = useAppStore((s) => s.selectedInterval)
    const colorMode = useAppStore((s) => s.colorMode)
    const splitRatio = useAppStore((s) => s.dashboardLayout.splitRatio)
    const setSplitRatio = useAppStore((s) => s.setDashboardSplitRatio)
    const selectedRange = useAppStore((s) => s.dashboardLayout.selectedRange)
    const setSelectedRange = useAppStore((s) => s.setDashboardSelectedRange)
    const indicatorVisibleMap = useAppStore((s) => s.dashboardLayout.indicatorVisible)
    const setIndicatorVisible = useAppStore((s) => s.setDashboardIndicatorVisible)
    const leftPanelOrder = useAppStore((s) => s.dashboardLayout.leftPanelOrder)
    const leftPanelVisible = useAppStore((s) => s.dashboardLayout.leftPanelVisible)
    const setLeftPanelOrder = useAppStore((s) => s.setDashboardLeftPanelOrder)
    const setLeftPanelVisible = useAppStore((s) => s.setDashboardLeftPanelVisible)

    const [leftMenuOpen, setLeftMenuOpen] = useState(false)
    const leftMenuRef = useRef<HTMLDivElement>(null)
    const [draggingPanel, setDraggingPanel] = useState<DashboardLeftPanelId | null>(null)
    const [crosshairData, setCrosshairData] = useState<CrosshairData | null>(null)
    const containerRef = useRef<HTMLDivElement>(null)
    const [isDraggingSplitter, setIsDraggingSplitter] = useState(false)
    const [dragRatio, setDragRatio] = useState(splitRatio)

    const lookbackDays = useMemo(() => selectedRange === 'max' ? (MAX_LOOKBACK_DAYS_BY_INTERVAL[interval] ?? 180) : selectedRange, [selectedRange, interval])
    const { from_ms, to_ms } = useMemo(() => {
        const to = Date.now()
        return { from_ms: to - lookbackDays * 24 * 60 * 60 * 1000, to_ms: to }
    }, [lookbackDays])

    const visibleIndicators = useMemo(() => {
        const set = new Set<string>()
        if (indicatorVisibleMap.ma5) set.add('ma5')
        if (indicatorVisibleMap.ma20) set.add('ma20')
        if (indicatorVisibleMap.ma50) set.add('ma50')
        if (indicatorVisibleMap.bollinger) set.add('bollinger')
        return set
    }, [indicatorVisibleMap])

    useEffect(() => chartSync.subscribeCrosshairData(setCrosshairData), [chartSync])

    useEffect(() => {
        const onOutside = (e: MouseEvent) => {
            const target = e.target as Node
            if (leftMenuRef.current && !leftMenuRef.current.contains(target)) setLeftMenuOpen(false)
        }
        document.addEventListener('mousedown', onOutside)
        return () => document.removeEventListener('mousedown', onOutside)
    }, [])

    useEffect(() => {
        if (!isDraggingSplitter) return
        const onMove = (e: MouseEvent) => {
            if (!containerRef.current) return
            const rect = containerRef.current.getBoundingClientRect()
            setDragRatio(Math.min(0.85, Math.max(0.35, (e.clientX - rect.left) / rect.width)))
        }
        const onUp = () => {
            setSplitRatio(dragRatio)
            setIsDraggingSplitter(false)
        }
        window.addEventListener('mousemove', onMove)
        window.addEventListener('mouseup', onUp)
        return () => {
            window.removeEventListener('mousemove', onMove)
            window.removeEventListener('mouseup', onUp)
        }
    }, [isDraggingSplitter, dragRatio, setSplitRatio])

    const { data: symbolsData } = useSymbols()
    const selectedSymbolData = (symbolsData?.symbols ?? []).find((s) => s.symbol === symbol)
    const candlesQuery = useCandles({ symbol, interval, from_ms, to_ms, indicators: 'ma5,ma20,ma50,rsi,macd,bollinger' })
    const signalsQuery = useSignals({ symbol, from_ms, to_ms })
    const candles = candlesQuery.data?.candles ?? []
    const signals = signalsQuery.data?.signals ?? []
    const latest = candles.at(-1) ?? null
    const prev = candles.length > 1 ? candles[candles.length - 2] : null

    const source = crosshairData ?? {
        open: latest?.open ?? null, high: latest?.high ?? null, low: latest?.low ?? null, close: latest?.close ?? null,
        volume: latest?.volume ?? null, prevClose: prev?.close ?? null, indicators: latest?.indicators ?? {},
    }
    const change = source.close != null && source.prevClose != null ? source.close - source.prevClose : null
    const changePct = change != null && source.prevClose ? (change / source.prevClose) * 100 : null
    const rsi = source.indicators['rsi14'] as number | undefined
    const macd = source.indicators['macd'] as { macd_line: number; signal_line: number; histogram: number } | undefined
    const boll = source.indicators['bollinger'] as { upper: number; middle: number; lower: number } | undefined
    const rsiStatus = rsi == null ? '--' : rsi >= 70 ? 'Overbought' : rsi <= 30 ? 'Oversold' : 'Neutral'
    const macdTrend = macd == null ? '--' : macd.macd_line >= macd.signal_line ? 'Bullish' : 'Bearish'

    const leftPanels = leftPanelOrder.filter((id) => leftPanelVisible[id])
    const leftWidth = `${Math.round((isDraggingSplitter ? dragRatio : splitRatio) * 10000) / 100}%`
    const rightWidth = `${Math.round((1 - (isDraggingSplitter ? dragRatio : splitRatio)) * 10000) / 100}%`
    const hasRsi = candles.some((c: any) => c.indicators?.['rsi14'] != null)
    const hasMacd = candles.some((c: any) => c.indicators?.['macd'] != null)
    const showLatencyBanner = isDataSourceError(candlesQuery.error) || candlesQuery.data?.cached === true

    return (
        <div className="flex h-full flex-col">
            <header className="sticky top-0 z-10 border-b border-surface-border bg-surface-card/80 px-6 py-4 backdrop-blur-sm">
                <div className="flex flex-wrap items-center gap-3">
                    <SymbolSelector />
                    <IntervalSelector />
                    <div className="flex items-center gap-1 rounded-lg border border-surface-border bg-surface-card p-1">
                        {TIME_RANGES.map((r) => (
                            <button key={r.label} onClick={() => setSelectedRange(r.value)} className={clsx('rounded-md px-2.5 py-1 text-xs', selectedRange === r.value ? 'bg-brand-600 text-white' : 'text-slate-400 hover:bg-surface-hover')}>
                                {r.label}
                            </button>
                        ))}
                    </div>
                    <div className="ml-auto rounded-lg border border-surface-border bg-surface px-3 py-1.5 text-sm font-semibold text-slate-100">
                        {symbol} {selectedSymbolData?.name ? `· ${selectedSymbolData.name}` : ''}
                    </div>
                </div>
            </header>

            <section className="border-b border-surface-border px-6 py-3">
                <div className="flex gap-2 overflow-x-auto pb-1">
                    <Stat label="Close" value={fmt(source.close)} accent={change != null ? (change > 0 ? 'up' : change < 0 ? 'down' : undefined) : undefined} />
                    <Stat label="High" value={fmt(source.high)} />
                    <Stat label="Low" value={fmt(source.low)} />
                    <Stat label="Open" value={fmt(source.open)} />
                    <Stat label="Volume" value={fmt(source.volume, 0)} />
                    <Stat label="Change" value={change == null || changePct == null ? '--' : `${change >= 0 ? '+' : ''}${fmt(change)} (${fmt(changePct)}%)`} accent={change != null ? (change > 0 ? 'up' : change < 0 ? 'down' : undefined) : undefined} />
                    <Stat label="RSI" value={fmt(rsi)} />
                    <Stat label="RSI State" value={rsiStatus} />
                    <Stat label="MACD" value={fmt(macd?.macd_line, 4)} />
                    <Stat label="Signal" value={fmt(macd?.signal_line, 4)} />
                    <Stat label="Histogram" value={fmt(macd?.histogram, 4)} />
                    <Stat label="MACD Trend" value={macdTrend} />
                    <Stat label="BOLL U" value={fmt(boll?.upper)} />
                    <Stat label="BOLL M" value={fmt(boll?.middle)} />
                    <Stat label="BOLL L" value={fmt(boll?.lower)} />
                </div>
            </section>

            <section ref={containerRef} className="flex min-h-0 flex-1 overflow-hidden">
                <div className="min-w-0 overflow-y-auto px-6 py-5" style={{ width: leftWidth }}>
                    <div className="mb-3 flex items-center gap-2">
                        <div className="flex items-center gap-1.5 text-slate-400">
                            <IconChartLine size={16} stroke={1.5} />
                            <h2 className="text-xs uppercase tracking-wider font-medium">
                                盤勢與籌碼 (左側區)
                            </h2>
                        </div>
                        <div className="ml-auto flex items-center gap-2">
                            <IndicatorToggle
                                visible={visibleIndicators}
                                onChange={(next) => {
                                    setIndicatorVisible('ma5', next.has('ma5'));
                                    setIndicatorVisible('ma20', next.has('ma20'));
                                    setIndicatorVisible('ma50', next.has('ma50'));
                                    setIndicatorVisible('bollinger', next.has('bollinger'))
                                }}
                            />
                            <div className="relative" ref={leftMenuRef}>
                                <button onClick={() => setLeftMenuOpen((v) => !v)}
                                    className="group flex items-center gap-2 rounded border border-surface-border px-2.5 py-1.5 text-xs text-slate-300"
                                >
                                    <IconSettings
                                        size={16}
                                        stroke={1.5}
                                        className="text-slate-400 group-hover:text-white group-hover:rotate-45 transition-transform duration-200"
                                    />
                                    <span className="text-sm font-medium">顯/隱設定</span>
                                </button>

                                {leftMenuOpen && (
                                    <div className="absolute right-0 z-20 mt-1 min-w-[220px] rounded-md border border-surface-border bg-surface-card p-2 shadow-xl">
                                        {leftPanelOrder.map((id) => (
                                            <label key={id} className="flex items-center gap-2 px-1 py-1 text-xs">
                                                <input type="checkbox" checked={leftPanelVisible[id]} onChange={() => setLeftPanelVisible(id, !leftPanelVisible[id])} />
                                                <span>{LEFT_LABEL[id]}</span>
                                            </label>
                                        ))}
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>

                    {showLatencyBanner && <DataLatencyBanner />}

                    {candlesQuery.isLoading ? (
                        <Card className="flex h-[500px] items-center justify-center"><LoadingSpinner size="lg" label="Loading..." /></Card>
                    ) : candlesQuery.isError && !candlesQuery.data ? (
                        <Card className="flex h-[500px] items-center justify-center"><p className="text-sm text-slate-500">Load failed</p></Card>
                    ) : (
                        <div className="flex flex-col gap-3">
                            {leftPanels.map((id) => (
                                <div key={id} onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'move' }} onDrop={(e) => {
                                    e.preventDefault()
                                    const dragged = (e.dataTransfer.getData('text/plain') || draggingPanel) as DashboardLeftPanelId | null
                                    if (!dragged) return
                                    setLeftPanelOrder(reorderLeftPanels(leftPanelOrder, dragged, id))
                                    setDraggingPanel(null)
                                }}>
                                    {id === 'candles' && (
                                        <Card padding={false} className="overflow-hidden">
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-move items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
                                                <span className="text-xs font-medium uppercase tracking-wider text-slate-400">{symbol} · {interval.toUpperCase()}</span>
                                                <span className="text-xs text-slate-600">{candles.length} bars</span>
                                            </div>
                                            <CandleChart candles={candles} signals={signals} height={500} visibleIndicators={visibleIndicators} sync={chartSync} showTooltip={false} />
                                        </Card>
                                    )}
                                    {id === 'rsi' && hasRsi && (
                                        <Card padding={false} className="overflow-hidden">
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-move items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
                                                <span className="text-xs font-medium uppercase tracking-wider text-slate-400">{LEFT_LABEL[id]}</span>
                                                <span className="text-slate-500 text-[11px] flex items-center gap-1.5">
                                                    <span className="text-emerald-500">小於 30% 超賣</span>
                                                    <span>|</span>
                                                    <span className="text-rose-500">大於 70% 超買</span>
                                                </span>
                                            </div>
                                            <IndicatorPane candles={candles} type="rsi14" sync={chartSync} colorMode={colorMode} />
                                        </Card>
                                    )}
                                    {id === 'macd' && hasMacd && (
                                        <Card padding={false} className="overflow-hidden">
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-move items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
                                                <span className="text-xs font-medium uppercase tracking-wider text-slate-400">{LEFT_LABEL[id]}</span>
                                                <span className="text-slate-500 text-[11px] flex items-center gap-1.5">
                                                    <span>[</span>
                                                    <span className="text-rose-500">紅：快線 (DIF)</span>
                                                    <span>|</span>
                                                    <span className="text-emerald-500">綠：慢線 (DEA)</span>
                                                    <span>|</span>
                                                    <span className="text-slate-400">柱狀圖 (OSC)</span>
                                                    <span>]</span>
                                                </span>
                                            </div>
                                            <IndicatorPane candles={candles} type="macd" sync={chartSync} colorMode={colorMode} />
                                        </Card>
                                    )}
                                    {id === 'institutionalNetFlow' && (
                                        <Card padding={false} className="overflow-hidden">
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-move items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
                                                <span className="text-xs font-medium uppercase tracking-wider text-slate-400">{LEFT_LABEL[id]}</span>
                                            </div>
                                            <div className="py-10 text-center text-sm text-slate-500">Placeholder panel</div>
                                        </Card>
                                    )}
                                </div>
                            ))}
                        </div>
                    )}
                </div>

                <div className={clsx('relative w-2 shrink-0 cursor-col-resize border-x border-surface-border bg-surface-hover', isDraggingSplitter && 'bg-brand-500/40')} onMouseDown={() => { setDragRatio(splitRatio); setIsDraggingSplitter(true) }}>
                    <div className="absolute left-1/2 top-1/2 h-10 w-1 -translate-x-1/2 -translate-y-1/2 rounded bg-slate-500/70" />
                </div>

                <aside className="min-w-[300px] overflow-y-auto px-4 py-5" style={{ width: rightWidth }}>
                    <PredictionPanel signals={signals} />
                    <div className="mt-4">
                        <h2 className="mb-2 text-xs font-medium uppercase tracking-wider text-slate-400">Signals ({signals.length})</h2>
                        {signalsQuery.isLoading ? <LoadingSpinner label="Loading signals..." /> : <SignalList signals={signals} />}
                    </div>
                </aside>
            </section>

            {candlesQuery.isError && <ErrorToast error={candlesQuery.error} onRetry={() => candlesQuery.refetch()} onRedirect={router.push} />}
            {signalsQuery.isError && <ErrorToast error={signalsQuery.error} onRedirect={router.push} />}
        </div>
    )
}