'use client'
/**
 * CandleChart — TradingView Lightweight Charts integration.
 * Pure props-driven: accepts candles + signals, no API or store access.
 */
import { useEffect, useRef } from 'react'
import type { CandleItem, SignalItem } from '@/types/api.generated'
import { toTradingViewCandle, toTradingViewVolume } from '@/lib/utils'
import { Time } from 'lightweight-charts';

interface CandleChartProps {
    candles: CandleItem[]
    signals?: SignalItem[]
    height?: number
    showVolume?: boolean
}

// Indicator colors as per FRONTEND_SPEC.md
const INDICATOR_COLORS = {
    ma5: '#2196F3',
    ma20: '#FF9800',
    ma50: '#9C27B0',
    bollUpper: '#607D8B',
    bollMid: '#78909C',
    bollLower: '#607D8B',
    rsi: '#00BCD4',
    macdLine: '#F44336',
    signal: '#4CAF50',
    histPos: 'rgba(76,175,80,0.6)',
    histNeg: 'rgba(244,67,54,0.6)',
}

export function CandleChart({ candles, signals = [], height = 500, showVolume = true }: CandleChartProps) {
    const containerRef = useRef<HTMLDivElement>(null)
    const chartRef = useRef<ReturnType<typeof import('lightweight-charts')['createChart']> | null>(null)

    useEffect(() => {
        if (!containerRef.current || candles.length === 0) return

        let chart: ReturnType<typeof import('lightweight-charts')['createChart']> | null = null

        import('lightweight-charts').then(({ createChart, CrosshairMode, LineStyle }) => {
            if (!containerRef.current) return

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
                crosshair: { mode: CrosshairMode.Normal },
                rightPriceScale: { borderColor: '#1e2a3a' },
                timeScale: {
                    borderColor: '#1e2a3a',
                    timeVisible: true,
                    secondsVisible: false,
                },
            })
            chartRef.current = chart

            // ── Candlestick Series ──────────────────────────────────────────────────
            const candleSeries = chart.addCandlestickSeries({
                upColor: '#10b981',
                downColor: '#ef4444',
                borderUpColor: '#10b981',
                borderDownColor: '#ef4444',
                wickUpColor: '#10b981',
                wickDownColor: '#ef4444',
            })
            candleSeries.setData(candles.map(toTradingViewCandle))

            // ── Signal Markers ──────────────────────────────────────────────────────
            if (signals.length > 0) {
                const markers = signals.map((s) => ({
                    time: (s.timestamp_ms / 1000) as Time,
                    position: s.signal_type === 'BUY' ? 'belowBar' as const : 'aboveBar' as const,
                    color: s.signal_type === 'BUY' ? '#4CAF50' : '#F44336',
                    shape: s.signal_type === 'BUY' ? 'arrowUp' as const : 'arrowDown' as const,
                    text: `${s.signal_type} ${(s.confidence * 100).toFixed(0)}%`,
                    size: s.reliability === 'low' ? 1 : 2,
                }))
                candleSeries.setMarkers(markers)
            }

            // ── MA Lines ────────────────────────────────────────────────────────────
            const firstCandle = candles[0]
            if (firstCandle.indicators['ma5'] !== undefined) {
                const ma5 = chart.addLineSeries({ color: INDICATOR_COLORS.ma5, lineWidth: 1, priceLineVisible: false })
                ma5.setData(candles
                    .filter((c) => c.indicators['ma5'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators['ma5'] as number })))
            }
            if (firstCandle.indicators['ma20'] !== undefined) {
                const ma20 = chart.addLineSeries({ color: INDICATOR_COLORS.ma20, lineWidth: 1, priceLineVisible: false })
                ma20.setData(candles
                    .filter((c) => c.indicators['ma20'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators['ma20'] as number })))
            }
            if (firstCandle.indicators['ma50'] !== undefined) {
                const ma50 = chart.addLineSeries({ color: INDICATOR_COLORS.ma50, lineWidth: 1, priceLineVisible: false })
                ma50.setData(candles
                    .filter((c) => c.indicators['ma50'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators['ma50'] as number })))
            }

            // ── Bollinger Bands ─────────────────────────────────────────────────────
            if (firstCandle.indicators['bollinger'] !== undefined) {
                const lineOpts = { color: INDICATOR_COLORS.bollMid, lineWidth: 1 as const, lineStyle: LineStyle.Dashed, priceLineVisible: false }
                const upper = chart.addLineSeries({ ...lineOpts, color: INDICATOR_COLORS.bollUpper })
                const mid = chart.addLineSeries({ ...lineOpts })
                const lower = chart.addLineSeries({ ...lineOpts, color: INDICATOR_COLORS.bollLower })

                candles.filter((c) => c.indicators['bollinger'] != null).forEach((c) => {
                    const b = c.indicators['bollinger'] as { upper: number; middle: number; lower: number }
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
                volSeries.setData(candles.map(toTradingViewVolume))
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

        return () => { chart?.remove() }
    }, [candles, signals, height, showVolume])

    return <div ref={containerRef} style={{ height }} className="w-full rounded-lg overflow-hidden" />
}

// ── IndicatorPane (RSI / MACD sub-charts) ─────────────────────────────────────

interface IndicatorPaneProps {
    candles: CandleItem[]
    type: 'rsi' | 'macd'
}

export function IndicatorPane({ candles, type }: IndicatorPaneProps) {
    const containerRef = useRef<HTMLDivElement>(null)
    const height = type === 'rsi' ? 100 : 120

    useEffect(() => {
        if (!containerRef.current || candles.length === 0) return
        let chart: ReturnType<typeof import('lightweight-charts')['createChart']> | null = null

        import('lightweight-charts').then(({ createChart }) => {
            if (!containerRef.current) return

            chart = createChart(containerRef.current, {
                width: containerRef.current.clientWidth,
                height,
                layout: { background: { color: '#161b27' }, textColor: '#94a3b8' },
                grid: { vertLines: { color: '#1e2a3a' }, horzLines: { color: '#1e2a3a' } },
                rightPriceScale: { borderColor: '#1e2a3a' },
                timeScale: { borderColor: '#1e2a3a', timeVisible: true, secondsVisible: false },
            })

            if (type === 'rsi') {
                const rsiSeries = chart.addLineSeries({ color: INDICATOR_COLORS.rsi, lineWidth: 1, priceLineVisible: false })
                rsiSeries.setData(candles
                    .filter((c) => c.indicators['rsi'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators['rsi'] as number })))
            } else {
                const macdLine = chart.addLineSeries({ color: INDICATOR_COLORS.macdLine, lineWidth: 1, priceLineVisible: false })
                const signalLine = chart.addLineSeries({ color: INDICATOR_COLORS.signal, lineWidth: 1, priceLineVisible: false })
                const histogram = chart.addHistogramSeries({ priceLineVisible: false })

                candles.filter((c) => c.indicators['macd'] != null).forEach((c) => {
                    const m = c.indicators['macd'] as { macd_line: number; signal_line: number; histogram: number }
                    const t = c.timestamp_ms / 1000
                    macdLine.update({ time: t as Time, value: m.macd_line })
                    signalLine.update({ time: t as Time, value: m.signal_line })
                    histogram.update({ time: t as Time, value: m.histogram, color: m.histogram >= 0 ? INDICATOR_COLORS.histPos : INDICATOR_COLORS.histNeg })
                })
            }
        })

        return () => { chart?.remove() }
    }, [candles, type, height])

    return (
        <div className="w-full">
            <div className="px-3 py-1 text-xs text-slate-500 uppercase tracking-wider font-medium">
                {type === 'rsi' ? 'RSI (14)' : 'MACD (12,26,9)'}
            </div>
            <div ref={containerRef} style={{ height }} className="w-full" />
        </div>
    )
}