'use client'
import { useState, useEffect } from 'react'
import { apiClient, buildQueryString } from '@/src/lib/api-client'
import { CandleChart } from '@/src/components/charts/CandleChart'
import { Card, LoadingSpinner, ErrorToast, Button } from '@/src/components/ui'
import type { CandleItem, SignalItem, CandlesResponse } from '@/src/types/api.types'
import { IndicatorToggle } from '@/src/components/charts/IndicatorToggle'

interface BacktestChartProps {
    symbol: string
    strategyName: string
    from_ms: number
    to_ms: number
    exitFilterPct?: number
}

function shouldHoldPosition(strategyName: string, candles: CandleItem[], idx: number): boolean {
    const close = candles[idx]?.close ?? 0
    const prev = candles[idx - 1]?.close ?? 0
    if (close <= 0 || prev <= 0) return false

    switch (strategyName) {
        case 'trend_follow_v1': {
            if (idx < 20) return false
            const ma5 = candles.slice(idx - 5, idx).reduce((s, c) => s + c.close, 0) / 5
            const ma20 = candles.slice(idx - 20, idx).reduce((s, c) => s + c.close, 0) / 20
            return ma5 > ma20
        }
        case 'mean_reversion_v1':
            return close < prev * 0.99
        case 'breakout_v1': {
            const start = Math.max(0, idx - 5)
            const recentMax = candles
                .slice(start, idx)
                .reduce((maxClose, candle) => Math.max(maxClose, candle.close), Number.NEGATIVE_INFINITY)
            return close > recentMax
        }
        default:
            return close > prev
    }
}

function buildBacktestTradeSignals(candles: CandleItem[], strategyName: string, exitFilterThreshold: number): SignalItem[] {
    if (candles.length < 2) return []

    let inPosition = false
    const markers: SignalItem[] = []

    for (let idx = 1; idx < candles.length - 1; idx += 1) {
        const execCandle = candles[idx + 1]
        const hold = shouldHoldPosition(strategyName, candles, idx)
        const close = candles[idx].close
        const prev = candles[idx - 1].close
        const dropRatio = prev > 0 ? (prev - close) / prev : 0
        const shouldExit = !hold && dropRatio >= exitFilterThreshold

        if (hold && !inPosition) {
            inPosition = true
            markers.push({
                id: `bt-buy-${execCandle.timestamp_ms}-${idx}`,
                timestamp_ms: execCandle.timestamp_ms,
                signal_type: 'BUY',
                confidence: 1,
                entry_price: execCandle.close,
                target_price: execCandle.close,
                stop_loss: execCandle.close,
                reason: `backtest:${strategyName}:entry`,
                source: 'technical_only',
                reliability: 'high',
                fallback_reason: null,
            })
        } else if (shouldExit && inPosition) {
            inPosition = false
            markers.push({
                id: `bt-sell-${execCandle.timestamp_ms}-${idx}`,
                timestamp_ms: execCandle.timestamp_ms,
                signal_type: 'SELL',
                confidence: 1,
                entry_price: execCandle.close,
                target_price: execCandle.close,
                stop_loss: execCandle.close,
                reason: `backtest:${strategyName}:exit`,
                source: 'technical_only',
                reliability: 'high',
                fallback_reason: null,
            })
        }
    }

    if (inPosition) {
        const last = candles[candles.length - 1]
        markers.push({
            id: `bt-sell-final-${last.timestamp_ms}`,
            timestamp_ms: last.timestamp_ms,
            signal_type: 'SELL',
            confidence: 1,
            entry_price: last.close,
            target_price: last.close,
            stop_loss: last.close,
            reason: `backtest:${strategyName}:force-exit`,
            source: 'technical_only',
            reliability: 'high',
            fallback_reason: null,
        })
    }

    return markers
}

export function BacktestChart({ symbol, strategyName, from_ms, to_ms, exitFilterPct = 1.5 }: BacktestChartProps) {
    const [candles, setCandles] = useState<CandleItem[]>([])
    const [signals, setSignals] = useState<SignalItem[]>([])
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<unknown>(null)
    const [hasMore, setHasMore] = useState(false)
    const [cursor, setCursor] = useState<string | null>(null)
    const [loadingMore, setLoadingMore] = useState(false)
    const [visibleIndicators, setVisibleIndicators] = useState<Set<string>>(new Set())

    // Initial load
    useEffect(() => {
        setCandles([])
        setSignals([])
        setCursor(null)
        setHasMore(false)
        setLoading(true)
        setError(null)

        apiClient<CandlesResponse>(
            `/api/v1/candles/${symbol}${buildQueryString({ from_ms, to_ms, interval: '1d', indicators: 'ma5,ma20,ma50,bollinger', })}`
        )
            .then((candlesRes) => {
                setCandles(candlesRes.candles)
                setSignals(buildBacktestTradeSignals(candlesRes.candles, strategyName, exitFilterPct / 100))
                setHasMore(candlesRes.next_cursor != null)
                setCursor(candlesRes.next_cursor ?? null)
            })
            .catch(setError)
            .finally(() => setLoading(false))
    }, [symbol, strategyName, from_ms, to_ms])

    // Load next page via cursor pagination
    async function loadMore() {
        if (!cursor) return
        setLoadingMore(true)
        try {
            const res = await apiClient<CandlesResponse>(
                `/api/v1/candles/${symbol}${buildQueryString({ from_ms, to_ms, interval: '1d', cursor, indicators: 'ma5,ma20,ma50,bollinger', })}`
            )
            setCandles((prev) => {
                const merged = [...prev, ...res.candles]
                setSignals(buildBacktestTradeSignals(merged, strategyName, exitFilterPct / 100))
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
                height={400}
                showVolume={false}
                markerTextMode="signalOnly"
                visibleIndicators={visibleIndicators}
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