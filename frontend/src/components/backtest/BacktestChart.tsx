'use client'
import { useState, useEffect } from 'react'
import { apiClient, buildQueryString } from '@/src/lib/api-client'
import { CandleChart } from '@/src/components/charts/CandleChart'
import { Card, LoadingSpinner, ErrorToast, Button } from '@/src/components/ui'
import type { CandleItem, SignalItem, CandlesResponse, TradeRecord } from '@/src/types/api.types'
import { IndicatorToggle } from '@/src/components/charts/IndicatorToggle'
import { useChartSync } from '@/src/hooks'

interface BacktestChartProps {
    symbol: string
    strategyName: string
    from_ms: number
    to_ms: number
    exitFilterPct?: number
    trades: TradeRecord[]
}

// 把 trades 轉成 SignalItem
function tradesToSignals(trades: TradeRecord[]): SignalItem[] {
    return trades.flatMap((trade) => [
        {
            id: `bt-buy-${trade.entry_timestamp_ms}`,
            timestamp_ms: trade.entry_timestamp_ms,
            signal_type: 'BUY' as const,
            confidence: 1,
            entry_price: trade.entry_price,
            target_price: trade.entry_price,
            stop_loss: trade.entry_price,
            reason: 'backtest:entry',
            source: 'technical_only' as const,
            reliability: 'high' as const,
            fallback_reason: null,
        },
        {
            id: `bt-sell-${trade.exit_timestamp_ms}`,
            timestamp_ms: trade.exit_timestamp_ms,
            signal_type: 'SELL' as const,
            confidence: 1,
            entry_price: trade.exit_price,
            target_price: trade.exit_price,
            stop_loss: trade.exit_price,
            reason: 'backtest:exit',
            source: 'technical_only' as const,
            reliability: trade.is_win ? 'high' as const : 'low' as const,
            fallback_reason: null,
        },
    ])
}

export function BacktestChart({ symbol, strategyName, from_ms, to_ms, exitFilterPct = 1.5, trades, }: BacktestChartProps) {
    const [candles, setCandles] = useState<CandleItem[]>([])
    const [signals, setSignals] = useState<SignalItem[]>([])
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<unknown>(null)
    const [hasMore, setHasMore] = useState(false)
    const [cursor, setCursor] = useState<string | null>(null)
    const [loadingMore, setLoadingMore] = useState(false)
    const ALL_INDICATORS = new Set(['ma5', 'ma20', 'ma50', 'bollinger'])
    const [visibleIndicators, setVisibleIndicators] = useState<Set<string>>(ALL_INDICATORS)
    const chartSync = useChartSync()

    // Initial load
    useEffect(() => {
        setCandles([])
        setSignals([])
        setCursor(null)
        setHasMore(false)
        setLoading(true)
        setError(null)
        setSignals(tradesToSignals(trades))

        apiClient<CandlesResponse>(
            `/api/v1/candles/${symbol}${buildQueryString({ from_ms, to_ms, interval: '1d', indicators: 'ma5,ma20,ma50,rsi,macd,bollinger', })}`
        )
            .then((candlesRes) => {
                setCandles(candlesRes.candles)
                setSignals(tradesToSignals(trades))
                setHasMore(candlesRes.next_cursor != null)
                setCursor(candlesRes.next_cursor ?? null)
            })
            .catch(setError)
            .finally(() => setLoading(false))
    }, [symbol, strategyName, from_ms, to_ms, trades])

    // Load next page via cursor pagination
    async function loadMore() {
        if (!cursor) return
        setLoadingMore(true)
        try {
            const res = await apiClient<CandlesResponse>(
                `/api/v1/candles/${symbol}${buildQueryString({ from_ms, to_ms, interval: '1d', cursor, indicators: 'ma5,ma20,ma50,rsi,macd,bollinger', })}`
            )
            setCandles((prev) => {
                const merged = [...prev, ...res.candles]
                return merged
            })
            setHasMore(res.next_cursor != null)
            setCursor(res.next_cursor ?? null)
        } catch (err) {
            setError(err)
        } finally {
            setLoadingMore(false)
        }
    }

    if (loading) {
        return (
            <Card className="flex items-center justify-center h-[200px]">
                <LoadingSpinner label="載入回測 K 線..." />
            </Card>
        )
    }

    if (error) {
        return (
            <>
                <Card className="flex items-center justify-center h-[200px]">
                    <p className="text-sm text-slate-500">K 線載入失敗</p>
                </Card>
                <ErrorToast error={error} onRetry={() => { setError(null); setLoading(true) }} />
            </>
        )
    }

    return (
        <Card padding={false} className="overflow-hidden">
            <div className="px-4 py-3 border-b border-surface-border flex items-center justify-between">
                <span className="text-xs font-medium text-slate-400 uppercase tracking-wider">
                    {symbol} · 回測期間 K 線
                </span>
                <div className="flex items-center gap-3">
                    <IndicatorToggle
                        visible={visibleIndicators}
                        onChange={setVisibleIndicators}
                    />
                    <span className="text-xs text-slate-600">{candles.length} 根</span>
                </div>
            </div>

            <CandleChart
                candles={candles}
                signals={signals}
                height={450}
                showVolume={false}
                markerTextMode="signalOnly"
                visibleIndicators={visibleIndicators}
                sync={chartSync}
            />

            {hasMore && (
                <div className="px-4 py-3 border-t border-surface-border flex justify-center">
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={loadMore}
                        loading={loadingMore}
                    >
                        載入更多 K 線
                    </Button>
                </div>
            )}
        </Card>
    )
}