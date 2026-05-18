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
import type { DashboardLeftPanelId, DashboardRightGridPreset, DashboardRightWidgetId } from '@/src/types/api.types'

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

const RIGHT_WIDGET_META: Record<DashboardRightWidgetId, { title: string; subtitle: string }> = {
    aiPrediction: { title: 'AI 預測概覽', subtitle: '現有' },
    shareholdingRatio: { title: '股權持股比例', subtitle: '待接入資料' },
    monthlyRevenue: { title: '月營收明細', subtitle: '待接入資料' },
    peAnalysis: { title: '本益比分析', subtitle: '待接入資料' },
    signalList: { title: '訊號清單', subtitle: '' },
}

const RIGHT_GRID_PRESETS: DashboardRightGridPreset[] = ['1x1', '2x2', '3x3']

function getRightColumns(preset: DashboardRightGridPreset): number {
    if (preset === '1x1') return 1
    if (preset === '2x2') return 2
    return 3
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
    const rightGridPreset = useAppStore((s) => s.dashboardLayout.rightGridPreset)
    const setRightGridPreset = useAppStore((s) => s.setDashboardRightGridPreset)
    const rightWidgets = useAppStore((s) => s.dashboardLayout.rightWidgets)
    const setRightWidgetVisible = useAppStore((s) => s.setDashboardRightWidgetVisible)
    const setRightWidgets = useAppStore((s) => s.setDashboardRightWidgets)

    const [leftMenuOpen, setLeftMenuOpen] = useState(false)
    const leftMenuRef = useRef<HTMLDivElement>(null)
    const [draggingPanel, setDraggingPanel] = useState<DashboardLeftPanelId | null>(null)
    const [crosshairData, setCrosshairData] = useState<CrosshairData | null>(null)

    const [rightMenuOpen, setRightMenuOpen] = useState(false)
    const rightMenuRef = useRef<HTMLDivElement>(null)
    const [draggingRight, setDraggingRight] = useState<DashboardRightWidgetId | null>(null)
    // 右側 widget 高度 state（以 id 為 key）
    const [widgetHeights, setWidgetHeights] = useState<Record<string, number>>({
        aiPrediction: 240, shareholdingRatio: 200, monthlyRevenue: 200, peAnalysis: 200, signalList: 320,
    })
    // 右側 widget 寬度倍數（1 = 1欄, 2 = 2欄）
    const [widgetSpans, setWidgetSpans] = useState<Record<string, number>>({
        aiPrediction: 1, shareholdingRatio: 1, monthlyRevenue: 1, peAnalysis: 1, signalList: 1,
    })

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
        const onOutside = (e: MouseEvent) => {
            if (rightMenuRef.current && !rightMenuRef.current.contains(e.target as Node))
                setRightMenuOpen(false)
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

    const rightColumns = getRightColumns(rightGridPreset)

    function makeResizeHandler(
        id: string,
        initialH: number,
        initialW: number,
        corner: 'bottom' | 'right' | 'corner',
        columns: number,
    ) {
        return (e: React.MouseEvent) => {
            e.preventDefault()
            const startX = e.clientX
            const startY = e.clientY
            const onMove = (ev: MouseEvent) => {
                const dy = ev.clientY - startY
                const dx = ev.clientX - startX
                if (corner === 'bottom' || corner === 'corner') {
                    setWidgetHeights((prev) => ({ ...prev, [id]: Math.max(120, initialH + dy) }))
                }
                if (corner === 'right' || corner === 'corner') {
                    // 每 120px 算一欄
                    const newSpan = Math.min(columns, Math.max(1, initialW + Math.round(dx / 120)))
                    setWidgetSpans((prev) => ({ ...prev, [id]: newSpan }))
                }
            }
            const onUp = () => {
                window.removeEventListener('mousemove', onMove)
                window.removeEventListener('mouseup', onUp)
            }
            window.addEventListener('mousemove', onMove)
            window.addEventListener('mouseup', onUp)
        }
    }

    const [rightOrder, setRightOrder] = useState<DashboardRightWidgetId[]>(
        rightWidgets.map((w) => w.id)
    )


    function reorderRightWidgets(
        fromId: DashboardRightWidgetId,
        toId: DashboardRightWidgetId,
    ) {
        if (fromId === toId) return
        setRightOrder((prev) => {
            const next = [...prev]
            const fi = next.indexOf(fromId)
            const ti = next.indexOf(toId)
            if (fi < 0 || ti < 0) return prev
            const [moved] = next.splice(fi, 1)
            next.splice(ti, 0, moved)
            return next
        })
    }

    const visibleRightWidgets = rightOrder
        .map((id) => rightWidgets.find((w) => w.id === id))
        .filter((w): w is typeof rightWidgets[0] => !!w && w.visible)

    useEffect(() => {
        chartSync.setSymbol()
    }, [symbol])

    useEffect(() => {
        if (!candlesQuery.isSuccess || candles.length === 0) return
        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                chartSync.markDataReady()
            })
        })
    }, [candlesQuery.isSuccess, symbol])

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
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-grab active:cursor-grabbing items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
                                                <span className="text-xs font-medium uppercase tracking-wider text-slate-400">{symbol} · {interval.toUpperCase()}</span>
                                                <span className="text-xs text-slate-600">{candles.length} bars</span>
                                            </div>
                                            <CandleChart candles={candles} signals={signals} visibleIndicators={visibleIndicators} sync={chartSync} showTooltip={false} />
                                        </Card>
                                    )}
                                    {id === 'rsi' && hasRsi && (
                                        <Card padding={false} className="overflow-hidden">
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-grab active:cursor-grabbing items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
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
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-grab active:cursor-grabbing items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
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
                                            <div draggable onDragStart={(e) => { setDraggingPanel(id); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id) }} onDragEnd={() => setDraggingPanel(null)} className="flex cursor-grab active:cursor-grabbing items-center justify-between border-b border-surface-border px-4 py-3 hover:bg-surface-hover/60">
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
                    {/* ── 右側頂部工具列 ── */}
                    <div className="mb-3 flex items-center gap-2">
                        <h2 className="text-xs uppercase tracking-wider text-slate-400 flex-1">財務資訊 (右側區)</h2>

                        {/* 格線切換 */}
                        <div className="flex items-center gap-1 rounded-md border border-surface-border bg-surface-card p-0.5">
                            {RIGHT_GRID_PRESETS.map((preset) => (
                                <button
                                    key={preset}
                                    onClick={() => setRightGridPreset(preset)}
                                    className={clsx(
                                        'rounded px-2 py-1 text-[11px] transition',
                                        rightGridPreset === preset
                                            ? 'bg-brand-600 text-white'
                                            : 'text-slate-400 hover:bg-surface-hover hover:text-slate-200',
                                    )}
                                >
                                    {preset}
                                </button>
                            ))}
                        </div>

                        {/* 顯/隱設定 */}
                        <div className="relative" ref={rightMenuRef}>
                            <button
                                onClick={() => setRightMenuOpen((v) => !v)}
                                className="group flex items-center gap-1.5 rounded border border-surface-border px-2.5 py-1.5 text-xs text-slate-300 hover:bg-surface-hover"
                            >
                                <IconSettings size={14} stroke={1.5} className="text-slate-400 group-hover:text-white transition-transform group-hover:rotate-45 duration-200" />
                                <span>顯/隱</span>
                            </button>
                            {rightMenuOpen && (
                                <div className="absolute right-0 z-20 mt-1 min-w-[200px] rounded-md border border-surface-border bg-surface-card p-2 shadow-xl">
                                    {rightWidgets.map((w) => (
                                        <label key={w.id} className="flex items-center gap-2 px-1 py-1 text-xs cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={w.visible}
                                                onChange={() => setRightWidgetVisible(w.id, !w.visible)}
                                            />
                                            <span className="text-slate-200">{RIGHT_WIDGET_META[w.id].title}</span>
                                        </label>
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* ── Widget 格線 ── */}
                    <div
                        className="grid gap-3"
                        style={{ gridTemplateColumns: `repeat(${rightColumns}, minmax(0, 1fr))` }}
                    >
                        {visibleRightWidgets.map((widget) => {
                            const id = widget.id
                            const h = widgetHeights[id] ?? 200
                            const span = Math.min(widgetSpans[id] ?? 1, rightColumns)

                            return (
                                <div
                                    key={id}
                                    style={{ gridColumn: `span ${span}` }}
                                    className={clsx(
                                        'relative rounded-xl border border-surface-border bg-surface-card overflow-visible',
                                        draggingRight === id && 'border-brand-500/40 bg-brand-500/5',
                                    )}
                                    onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'move' }}
                                    onDrop={(e) => {
                                        e.preventDefault()
                                        const dragged = (e.dataTransfer.getData('text/plain') || draggingRight) as DashboardRightWidgetId | null
                                        if (dragged) { reorderRightWidgets(dragged, id); setDraggingRight(null) }
                                    }}
                                >
                                    {/* Header（可拖曳） */}
                                    <div
                                        draggable
                                        onDragStart={(e) => {
                                            setDraggingRight(id)
                                            e.dataTransfer.effectAllowed = 'move'
                                            e.dataTransfer.setData('text/plain', id)
                                        }}
                                        onDragEnd={() => setDraggingRight(null)}
                                        className="flex cursor-grab active:cursor-grabbing items-center gap-2 border-b border-surface-border px-3 py-2 hover:bg-surface-hover/60 rounded-t-xl"
                                    >
                                        <span className="text-xs font-medium uppercase tracking-wider text-slate-400 flex-1">
                                            {RIGHT_WIDGET_META[id].title}
                                        </span>
                                        <span className="text-[11px] text-slate-600">
                                            {id === 'signalList'
                                                ? `${signals.length} 筆`
                                                : RIGHT_WIDGET_META[id].subtitle}
                                        </span>
                                    </div>

                                    {/* Body */}
                                    <div style={{ height: h }} className="overflow-auto p-3">
                                        {id === 'aiPrediction' && <PredictionPanel signals={signals} />}
                                        {id === 'shareholdingRatio' && (
                                            <div className="flex h-full items-center justify-center text-sm text-slate-500">
                                                股權持股比例：待接入資料
                                            </div>
                                        )}
                                        {id === 'monthlyRevenue' && (
                                            <div className="flex h-full items-center justify-center text-sm text-slate-500">
                                                月營收明細：待接入資料
                                            </div>
                                        )}
                                        {id === 'peAnalysis' && (
                                            <div className="flex h-full items-center justify-center text-sm text-slate-500">
                                                本益比分析：待接入資料
                                            </div>
                                        )}
                                        {id === 'signalList' && (
                                            signalsQuery.isLoading
                                                ? <LoadingSpinner label="載入訊號..." />
                                                : <SignalList signals={signals} />
                                        )}
                                    </div>

                                    {/* 底部 resize handle */}
                                    <div
                                        onMouseDown={makeResizeHandler(id, h, widgetSpans[id] ?? 1, 'bottom', rightColumns)}
                                        className="absolute bottom-0 left-4 right-4 h-1.5 cursor-row-resize hover:bg-brand-500/40 transition-colors rounded-b"
                                    />
                                    {/* 右側 resize handle */}
                                    <div
                                        onMouseDown={makeResizeHandler(id, h, widgetSpans[id] ?? 1, 'right', rightColumns)}
                                        className="absolute right-0 top-8 bottom-4 w-1.5 cursor-col-resize hover:bg-brand-500/40 transition-colors rounded-r"
                                    />
                                    {/* 右下角 resize handle */}
                                    <div
                                        onMouseDown={makeResizeHandler(id, h, widgetSpans[id] ?? 1, 'corner', rightColumns)}
                                        className="absolute bottom-0 right-0 w-3 h-3 cursor-nwse-resize hover:bg-brand-500/60 transition-colors rounded-br"
                                    />
                                </div>
                            )
                        })}
                    </div>
                </aside>
            </section>

            {candlesQuery.isError && <ErrorToast error={candlesQuery.error} onRetry={() => candlesQuery.refetch()} onRedirect={router.push} />}
            {signalsQuery.isError && <ErrorToast error={signalsQuery.error} onRedirect={router.push} />}
        </div>
    )
}