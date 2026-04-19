'use client'
import { useState, useMemo } from 'react'
import { useRouter } from 'next/navigation'
import { clsx } from 'clsx'
import { useCandles, useSignals } from '@/src/hooks'
import { useAppStore } from '@/src/store/useAppStore'
import { CandleChart, IndicatorPane } from '@/src/components/charts'
import {
    SymbolSelector,
    IntervalSelector,
    SignalList,
    PredictionPanel,
} from '@/src/components/dashboard'
import { LoadingSpinner, ErrorToast, Card } from '@/src/components/ui'
import { ApiErrorException } from '@/src/lib/api-client'

// ── Time range options ────────────────────────────────────────────────────────

const TIME_RANGES = [
    { label: '1D', days: 1 },
    { label: '5D', days: 5 },
    { label: '1M', days: 30 },
    { label: '3M', days: 90 },
    { label: '6M', days: 180 },
]

function useTimeRange() {
    const [days, setDays] = useState(7)
    const range = useMemo(() => {
        const to = Date.now()
        const from = to - days * 24 * 60 * 60 * 1000
        return { from_ms: from, to_ms: to }
    }, [days])
    return { days, setDays, ...range }
}

// ── Data latency banner ───────────────────────────────────────────────────────

function DataLatencyBanner() {
    return (
        <div className="flex items-center gap-2 px-3 py-2 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-400">
            <span>⚠</span>
            <span>資料可能延遲 — 數據源暫中斷，顯示快取數據</span>
        </div>
    )
}

function isDataSourceError(error: unknown): boolean {
    return (
        error instanceof ApiErrorException &&
        (error.errorCode === 'DATA_SOURCE_INTERRUPTED' ||
            error.errorCode === 'DATA_SOURCE_RATE_LIMITED')
    )
}

// ── Page ──────────────────────────────────────────────────────────────────────

export default function DashboardPage() {
    const router = useRouter()
    const symbol = useAppStore((s) => s.selectedSymbol)
    const interval = useAppStore((s) => s.selectedInterval)
    const { days, setDays, from_ms, to_ms } = useTimeRange()

    const candlesQuery = useCandles({
        symbol, interval, from_ms, to_ms,
        indicators: 'ma5,ma20,ma50,rsi,macd,bollinger',
    })
    const signalsQuery = useSignals({ symbol, from_ms, to_ms })

    const candles = candlesQuery.data?.candles ?? []
    const signals = signalsQuery.data?.signals ?? []
    const isLoading = candlesQuery.isLoading

    const showLatencyBanner =
        isDataSourceError(candlesQuery.error) ||
        candlesQuery.data?.cached === true

    return (
        <div className="flex flex-col h-full">

            {/* Top bar */}
            <header className="flex items-center gap-4 px-6 py-4 border-b border-surface-border bg-surface-card/50 backdrop-blur-sm sticky top-0 z-10">
                <div>
                    <h1 className="text-base font-semibold text-slate-100">Dashboard</h1>
                    <p className="text-xs text-slate-500 mt-0.5">即時 K 線與交易信號</p>
                </div>
                <div className="flex items-center gap-3 ml-auto flex-wrap justify-end">
                    <SymbolSelector />
                    <IntervalSelector />

                    {/* Time range selector */}
                    <div className="flex items-center gap-1 bg-surface-card border border-surface-border rounded-lg p-1">
                        {TIME_RANGES.map((r) => (
                            <button
                                key={r.days}
                                onClick={() => setDays(r.days)}
                                className={clsx(
                                    'px-2.5 py-1 rounded-md text-xs font-medium transition-all',
                                    days === r.days
                                        ? 'bg-brand-600 text-white'
                                        : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover',
                                )}
                            >
                                {r.label}
                            </button>
                        ))}
                    </div>

                    {candlesQuery.isFetching && !isLoading && (
                        <div className="w-4 h-4 border-2 border-brand-500/30 border-t-brand-500 rounded-full animate-spin" />
                    )}
                </div>
            </header>

            {/* Body */}
            <div className="flex flex-1 overflow-hidden">

                {/* Main chart column */}
                <div className="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-3">
                    {showLatencyBanner && <DataLatencyBanner />}

                    {isLoading ? (
                        <Card className="flex items-center justify-center h-[500px]">
                            <LoadingSpinner size="lg" label="載入 K 線資料..." />
                        </Card>
                    ) : candlesQuery.isError && !candlesQuery.data ? (
                        <Card className="flex items-center justify-center h-[500px]">
                            <p className="text-sm text-slate-500">圖表載入失敗，請重試</p>
                        </Card>
                    ) : (
                        <>
                            <Card padding={false} className="overflow-hidden">
                                <div className="px-4 py-3 border-b border-surface-border flex items-center justify-between">
                                    <span className="text-xs font-medium text-slate-400 uppercase tracking-wider">
                                        {symbol} · {interval.toUpperCase()}
                                    </span>
                                    <div className="flex items-center gap-3 text-xs">
                                        {showLatencyBanner && (
                                            <span className="text-amber-500 flex items-center gap-1">
                                                <span>⚠</span> 資料可能延遲
                                            </span>
                                        )}
                                        <span className="text-slate-600">{candles.length} 根 K 線</span>
                                    </div>
                                </div>
                                <CandleChart candles={candles} signals={signals} height={500} />
                            </Card>

                            {candles.some((c) => c.indicators['rsi'] != null) && (
                                <Card padding={false} className="overflow-hidden">
                                    <IndicatorPane candles={candles} type="rsi" />
                                </Card>
                            )}

                            {candles.some((c) => c.indicators['macd'] != null) && (
                                <Card padding={false} className="overflow-hidden">
                                    <IndicatorPane candles={candles} type="macd" />
                                </Card>
                            )}
                        </>
                    )}

                    {candlesQuery.isError && (
                        <ErrorToast error={candlesQuery.error} onRetry={() => candlesQuery.refetch()} onRedirect={router.push} />
                    )}
                    {signalsQuery.isError && (
                        <ErrorToast error={signalsQuery.error} onRedirect={router.push} />
                    )}
                </div>

                {/* Right sidebar */}
                <aside className="w-80 shrink-0 border-l border-surface-border overflow-y-auto px-4 py-5 flex flex-col gap-4">
                    <PredictionPanel signals={signals} />
                    <div>
                        <h2 className="text-xs font-medium text-slate-400 uppercase tracking-wider mb-3">
                            交易信號 ({signals.length})
                        </h2>
                        {signalsQuery.isLoading
                            ? <LoadingSpinner label="載入信號..." />
                            : <SignalList signals={signals} />
                        }
                    </div>
                </aside>
            </div>
        </div>
    )
}