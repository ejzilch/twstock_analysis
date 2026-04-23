'use client'
import { useEffect, useRef } from 'react'
import type { CandleItem, SignalItem } from '@/src/types/api.generated'
import { toTradingViewCandle, toTradingViewVolume } from '@/src/lib/utils'
import { Time } from 'lightweight-charts';
import { useAppStore } from '@/src/store/useAppStore'

interface CandleChartProps {
    candles: CandleItem[]
    signals?: SignalItem[]
    height?: number
    showVolume?: boolean
    markerTextMode?: 'signalWithConfidence' | 'signalOnly'
}

export function CandleChart({
    candles,
    signals = [],
    height = 500,
    showVolume = true,
    markerTextMode = 'signalWithConfidence',
}: CandleChartProps) {
    const containerRef = useRef<HTMLDivElement>(null)
    const chartRef = useRef<ReturnType<typeof import('lightweight-charts')['createChart']> | null>(null)
    const colorMode = useAppStore((s) => s.colorMode)

    useEffect(() => {
        if (!containerRef.current || candles.length === 0) return

        let isMounted = true

        let chart: ReturnType<typeof import('lightweight-charts')['createChart']> | null = null

        import('lightweight-charts').then(({ createChart, CrosshairMode, LineStyle }) => {
            if (!isMounted || !containerRef.current) return

            chart = createChart(containerRef.current, {
                width: containerRef.current.clientWidth,
                height,
                layout: {
                    background: { color: '#161b27' },
                    textColor: '#94a3b8',
                },
                grid: {
                    vertLines: { color: '#1e2a3a' },
                    horzLines: { color: '#1e2a3a' },
                },
                crosshair: { mode: CrosshairMode.Magnet },
                rightPriceScale: { borderColor: '#1e2a3a' },
                timeScale: {
                    borderColor: '#1e2a3a',
                    timeVisible: true,
                    secondsVisible: false,
                },
            })
            chartRef.current = chart

            // TW 模式：紅漲綠跌；US 模式：綠漲紅跌
            const upColor = colorMode === 'TW' ? '#ef4444' : '#10b981'
            const downColor = colorMode === 'TW' ? '#10b981' : '#ef4444'

            // ── Candlestick Series ──────────────────────────────────────────────────
            const candleSeries = chart.addCandlestickSeries({
                upColor,
                downColor,
                borderUpColor: upColor,
                borderDownColor: downColor,
                wickUpColor: upColor,
                wickDownColor: downColor,
            })
            candleSeries.setData(candles.map(toTradingViewCandle))

            // ── Signal Markers ──────────────────────────────────────────────────────
            if (signals.length > 0) {
                const firstCandleMs = candles[0].timestamp_ms
                const lastCandleMs = candles[candles.length - 1].timestamp_ms
                const markers = signals
                    .filter((s) => s.timestamp_ms >= firstCandleMs && s.timestamp_ms <= lastCandleMs)
                    .sort((a, b) => a.timestamp_ms - b.timestamp_ms)
                    .map((s) => ({
                        time: (s.timestamp_ms / 1000) as Time,
                        position: s.signal_type === 'BUY' ? 'belowBar' as const : 'aboveBar' as const,
                        color: s.signal_type === 'BUY' ? '#4CAF50' : '#F44336',
                        shape: s.signal_type === 'BUY' ? 'arrowUp' as const : 'arrowDown' as const,
                        text: markerTextMode === 'signalOnly'
                            ? s.signal_type
                            : `${s.signal_type} ${(s.confidence * 100).toFixed(0)}%`,
                        size: s.reliability === 'low' ? 1 : 2,
                    }))
                candleSeries.setMarkers(markers)
            }

            // ── MA Lines ────────────────────────────────────────────────────────────
            const hasMa5 = candles.some((c) => c.indicators?.['ma5'] != null)
            const hasMa20 = candles.some((c) => c.indicators?.['ma20'] != null)
            const hasMa50 = candles.some((c) => c.indicators?.['ma50'] != null)
            const hasBollinger = candles.some((c) => c.indicators?.['bollinger'] != null)
            if (hasMa5) {
                const ma5 = chart.addLineSeries({ color: INDICATOR_COLORS.ma5, lineWidth: 1, priceLineVisible: false })
                ma5.setData(candles
                    .filter((c) => c.indicators?.['ma5'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators?.['ma5'] as number })))
            }

            if (hasMa20) {
                const ma20 = chart.addLineSeries({ color: INDICATOR_COLORS.ma20, lineWidth: 1, priceLineVisible: false })
                ma20.setData(candles.filter((c) => c.indicators?.['ma20'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators?.['ma20'] as number })))
            }

            if (hasMa50) {
                const ma50 = chart.addLineSeries({ color: INDICATOR_COLORS.ma50, lineWidth: 1, priceLineVisible: false })
                ma50.setData(candles.filter((c) => c.indicators?.['ma50'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators?.['ma50'] as number })))
            }

            // ── Bollinger Bands ─────────────────────────────────────────────────────
            if (hasBollinger) {
                const lineOpts = { color: INDICATOR_COLORS.bollMid, lineWidth: 1 as const, lineStyle: LineStyle.Dashed, priceLineVisible: false }
                const upper = chart.addLineSeries({ ...lineOpts, color: INDICATOR_COLORS.bollUpper })
                const mid = chart.addLineSeries({ ...lineOpts })
                const lower = chart.addLineSeries({ ...lineOpts, color: INDICATOR_COLORS.bollLower })

                candles.filter((c) => c.indicators?.['bollinger'] != null).forEach((c) => {
                    const b = c.indicators?.['bollinger'] as { upper: number; middle: number; lower: number }
                    const t = c.timestamp_ms / 1000
                    upper.update({ time: t as Time, value: b.upper })
                    mid.update({ time: t as Time, value: b.middle })
                    lower.update({ time: t as Time, value: b.lower })
                })
            }

            // ── Volume ──────────────────────────────────────────────────────────────
            if (showVolume) {
                const volSeries = chart.addHistogramSeries({
                    priceFormat: { type: 'volume' },
                    priceScaleId: 'volume',
                })
                chart.priceScale('volume').applyOptions({ scaleMargins: { top: 0.85, bottom: 0 } })
                volSeries.setData(candles.map((c) => toTradingViewVolume(c, colorMode)))
            }

            // ── Responsive resize ───────────────────────────────────────────────────
            const resizeObserver = new ResizeObserver(() => {
                if (containerRef.current && chart) {
                    chart.applyOptions({ width: containerRef.current.clientWidth })
                }
            })
            if (containerRef.current) resizeObserver.observe(containerRef.current)

            return () => resizeObserver.disconnect()
        })

        return () => {
            isMounted = false
            chart?.remove()   // 若 .then() 已執行，這裡能正確拿到 chart
        }

    }, [candles, signals, height, showVolume, colorMode])

    return <div ref={containerRef} style={{ height }} className="w-full rounded-lg overflow-hidden" />
}

// ── IndicatorPane (RSI / MACD sub-charts) ─────────────────────────────────────

