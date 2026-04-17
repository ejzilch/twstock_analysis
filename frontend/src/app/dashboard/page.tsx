'use client'
import { useMemo } from 'react'
import { useRouter } from 'next/navigation'
import { useCandles, useSignals } from '@/hooks'
import { useAppStore } from '@/store/useAppStore'
import { CandleChart, IndicatorPane } from '@/components/charts/CandleChart'
import {
    SymbolSelector,
    IntervalSelector,
    SignalList,
    PredictionPanel,
} from '@/components/dashboard'
import { LoadingSpinner, ErrorToast, Card } from '@/components/ui'
import { isMarketOpen } from '@/lib/utils'

// Default: last 7 days
function defaultTimeRange() {
    const to = Date.now()
    const from = to - 7 * 24 * 60 * 60 * 1000
    return { from_ms: from, to_ms: to }
}

export default function DashboardPage() {
    const router = useRouter()
    const symbol = useAppStore((s) => s.selectedSymbol)
    const interval = useAppStore((s) => s.selectedInterval)
    const { from_ms, to_ms } = useMemo(defaultTimeRange, [])

    const candlesQuery = useCandles({
        symbol,
        interval,
        from_ms,
        to_ms,
        indicators: 'ma5,ma20,ma50,rsi,macd,bollinger',
    })

    const signalsQuery = useSignals({ symbol, from_ms, to_ms })

    const candles = candlesQuery.data?.candles ?? []
    const signals = signalsQuery.data?.signals ?? []
    const isLoading = candlesQuery.isLoading || signalsQuery.isLoading

    return (
        <div className="flex flex-col h-full">
            {/* ── Top Bar ─────────────────────────────────────────────────────── */}
            <header className="flex items-center gap-4 px-6 py-4 border-b border-surface-border bg-surface-card/50 backdrop-blur-sm sticky top-0 z-10">
                <div>
                    <h1 className="text-base font-semibold text-slate-100">Dashboard</h1>
                    <p className="text-xs text-slate-500 mt-0.5">
                        即時 K 線與交易信號
                        {isMarketOpen()
                            ? <span className="ml-2 text-emerald-400">● 交易中</span>
                            : <span className="ml-2 text-slate-600">● 休市</span>}
                    </p>
                </div>
                <div className="flex items-center gap-3 ml-auto">
                    <SymbolSelector />
                    <IntervalSelector />
                    {candlesQuery.isFetching && (
                        <div className="w-4 h-4 border-2 border-brand-500/30 border-t-brand-500 rounded-full animate-spin" />
                    )}
                </div>
            </header>

            {/* ── Body ────────────────────────────────────────────────────────── */}
            <div className="flex flex-1 overflow-hidden">
                {/* Main Chart Column */}
                <div className="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-3">
                    {isLoading ? (
                        <Card className="flex flex-col items-center justify-center h-[500px] bg-surface-card border-surface-border">
                            <LoadingSpinner size="lg" />
                            <p className="mt-4 text-sm text-slate-500 animate-pulse">載入 K 線資料...</p>
                        </Card>
                    ) : candlesQuery.isError ? (
                        <>
                            <Card className="flex items-center justify-center h-[500px] bg-surface-card border-surface-border">
                                <p className="text-sm text-slate-500">圖表載入失敗，請重試</p>
                            </Card>
                            <ErrorToast
                                error={candlesQuery.error}
                                onRetry={() => candlesQuery.refetch()}
                                onRedirect={router.push}
                            />
                        </>
                    ) : (
                        <>
                            {/* Data freshness warning */}
                            {candlesQuery.data?.cached && (
                                <div className="flex items-center gap-2 px-3 py-2 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-400">
                                    <span>⚠</span> 目前顯示快取資料，資料可能延遲
                                </div>
                            )}

                            {/* Main K-line Chart */}
                            <Card padding={false} className="overflow-hidden">
                                <div className="px-4 py-3 border-b border-surface-border flex items-center justify-between">
                                    <span className="text-xs font-medium text-slate-400 uppercase tracking-wider">
                                        {symbol} · {interval.toUpperCase()}
                                    </span>
                                    <span className="text-xs text-slate-600">
                                        {candles.length} 根 K 線
                                    </span>
                                </div>
                                <CandleChart candles={candles} signals={signals} height={500} />
                            </Card>

                            {/* RSI sub-chart */}
                            {candles.some((c) => c.indicators['rsi'] != null) && (
                                <Card padding={false} className="overflow-hidden">
                                    <IndicatorPane candles={candles} type="rsi" />
                                </Card>
                            )}

                            {/* MACD sub-chart */}
                            {candles.some((c) => c.indicators['macd'] != null) && (
                                <Card padding={false} className="overflow-hidden">
                                    <IndicatorPane candles={candles} type="macd" />
                                </Card>
                            )}
                        </>
                    )}

                    {signalsQuery.isError && (
                        <ErrorToast
                            error={signalsQuery.error}
                            onRedirect={router.push}
                        />
                    )}
                </div>

                {/* Right Sidebar — Signals */}
                <aside className="w-80 shrink-0 border-l border-surface-border overflow-y-auto px-4 py-5 flex flex-col gap-4">
                    <PredictionPanel signals={signals} />
                    <div>
                        <h2 className="text-xs font-medium text-slate-400 uppercase tracking-wider mb-3">
                            交易信號 ({signals.length})
                        </h2>
                        {signalsQuery.isLoading
                            ? <LoadingSpinner label="載入信號..." />
                            : <SignalList signals={signals} />}
                    </div>
                </aside>
            </div>
        </div>
    )
}