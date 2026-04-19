'use client'
/**
 * BacktestChart — K-line chart with signal overlay for backtest result page.
 * Uses cursor pagination to load candles in batches (respects 2000-bar limit).
 */
import { useState, useEffect } from 'react'
import { apiClient, buildQueryString } from '@/src/lib/api-client'
import { CandleChart } from '@/src/components/charts/CandleChart'
import { Card, LoadingSpinner, ErrorToast, Button } from '@/src/components/ui'
import type { CandleItem, SignalItem, CandlesResponse, SignalsResponse } from '@/src/types/api.generated'

interface BacktestChartProps {
    symbol: string
    from_ms: number
    to_ms: number
}

export function BacktestChart({ symbol, from_ms, to_ms }: BacktestChartProps) {
    const [candles, setCandles] = useState<CandleItem[]>([])
    const [signals, setSignals] = useState<SignalItem[]>([])
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<unknown>(null)
    const [hasMore, setHasMore] = useState(false)
    const [cursor, setCursor] = useState<string | null>(null)
    const [loadingMore, setLoadingMore] = useState(false)

    // Initial load
    useEffect(() => {
        setCandles([])
        setSignals([])
        setCursor(null)
        setHasMore(false)
        setLoading(true)
        setError(null)

        Promise.all([
            apiClient<CandlesResponse>(
                `/api/v1/candles/${symbol}${buildQueryString({ from_ms, to_ms, interval: '1d' })}`
            ),
            apiClient<SignalsResponse>(
                `/api/v1/signals/${symbol}${buildQueryString({ from_ms, to_ms })}`
            ),
        ])
            .then(([candlesRes, signalsRes]) => {
                setCandles(candlesRes.candles)
                setSignals(signalsRes.signals)
                setHasMore(candlesRes.next_cursor != null)
                setCursor(candlesRes.next_cursor)
            })
            .catch(setError)
            .finally(() => setLoading(false))
    }, [symbol, from_ms, to_ms])

    // Load next page via cursor pagination
    async function loadMore() {
        if (!cursor) return
        setLoadingMore(true)
        try {
            const res = await apiClient<CandlesResponse>(
                `/api/v1/candles/${symbol}${buildQueryString({ from_ms, to_ms, interval: '1d', cursor })}`
            )
            setCandles((prev) => [...prev, ...res.candles])
            setHasMore(res.next_cursor != null)
            setCursor(res.next_cursor)
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
                <span className="text-xs text-slate-600">{candles.length} 根</span>
            </div>

            <CandleChart candles={candles} signals={signals} height={200} showVolume={false} />

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