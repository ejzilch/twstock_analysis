'use client'
import { useEffect, useRef, useState } from 'react'
import type { CandleItem, SignalItem } from '@/src/types/api.generated'
import { Time, IChartApi, ISeriesApi } from 'lightweight-charts';
import { useAppStore } from '@/src/store/useAppStore'
import {
    INDICATOR_COLORS,
    CHART_THEME,
    SIGNAL_TYPE,
    getCandleColors,
} from '@/src/constants/chartColors'
import { ChartSyncHandle } from '@/src/hooks/useChartSync'

interface CandleChartProps {
    candles: CandleItem[]
    signals?: SignalItem[]
    height?: number
    showVolume?: boolean
    markerTextMode?: 'signalWithConfidence' | 'signalOnly'
    visibleIndicators?: Set<string>
    sync?: ChartSyncHandle
}

interface SeriesRefs {
    candle: ISeriesApi<'Candlestick'> | null
    volume: ISeriesApi<'Histogram'> | null
    ma5: ISeriesApi<'Line'> | null
    ma20: ISeriesApi<'Line'> | null
    ma50: ISeriesApi<'Line'> | null
    bollUpper: ISeriesApi<'Line'> | null
    bollMid: ISeriesApi<'Line'> | null
    bollLower: ISeriesApi<'Line'> | null
}

export function CandleChart({
    candles,
    signals = [],
    height = 500,
    showVolume = true,
    markerTextMode = 'signalWithConfidence',
    visibleIndicators,
    sync,
}: CandleChartProps) {
    const containerRef = useRef<HTMLDivElement>(null)
    const chartRef = useRef<IChartApi | null>(null)
    const seriesRef = useRef<SeriesRefs>({
        candle: null, volume: null,
        ma5: null, ma20: null, ma50: null,
        bollUpper: null, bollMid: null, bollLower: null,
    })
    const colorMode = useAppStore((s) => s.colorMode)

    // 記錄是否已對齊過
    const hasAlignedRef = useRef(false)
    const prevCandleLengthRef = useRef(0)
    const cancelAlignRef = useRef<(() => void) | null>(null)

    // ── 橋梁 state：Effect 1 完成後設為 true，通知 Effect 2 可以執行 ──────────
    const [chartReady, setChartReady] = useState(false)

    // ── Effect 1：重建 chart ──────────────────────────────────────────────────
    useEffect(() => {
        if (!containerRef.current) return

        // 重建前先把 chartReady 設回 false，避免 Effect 2 用到舊的 series
        setChartReady(false)

        let isMounted = true
        let resizeObserver: ResizeObserver | null = null

        import('lightweight-charts').then(({ createChart, CrosshairMode, LineStyle }) => {
            if (!isMounted || !containerRef.current) return

            chartRef.current?.remove()
            chartRef.current = null
            seriesRef.current = {
                candle: null, volume: null,
                ma5: null, ma20: null, ma50: null,
                bollUpper: null, bollMid: null, bollLower: null,
            }

            const chart = createChart(containerRef.current, {
                width: containerRef.current.clientWidth,
                height,
                layout: {
                    background: { color: CHART_THEME.background },
                    textColor: CHART_THEME.textColor,
                },
                grid: {
                    vertLines: { color: CHART_THEME.gridLine },
                    horzLines: { color: CHART_THEME.gridLine },
                },
                crosshair: { mode: CrosshairMode.Magnet },
                rightPriceScale: {
                    borderColor: CHART_THEME.borderColor,
                    minimumWidth: 80,
                },
                timeScale: {
                    borderColor: CHART_THEME.borderColor,
                    timeVisible: true,
                    secondsVisible: false,
                    rightOffset: 12,
                },
            })

            chartRef.current = chart

            seriesRef.current.candle = chart.addCandlestickSeries({
                priceLineVisible: false,
            })
            seriesRef.current.ma5 = chart.addLineSeries({ color: INDICATOR_COLORS.ma5, lineWidth: 1, priceLineVisible: false })
            seriesRef.current.ma20 = chart.addLineSeries({ color: INDICATOR_COLORS.ma20, lineWidth: 1, priceLineVisible: false })
            seriesRef.current.ma50 = chart.addLineSeries({ color: INDICATOR_COLORS.ma50, lineWidth: 1, priceLineVisible: false })

            const bollOpts = { lineWidth: 1 as const, lineStyle: LineStyle.Dashed, priceLineVisible: false }
            seriesRef.current.bollUpper = chart.addLineSeries({ ...bollOpts, color: INDICATOR_COLORS.bollUpper })
            seriesRef.current.bollMid = chart.addLineSeries({ ...bollOpts, color: INDICATOR_COLORS.bollMid })
            seriesRef.current.bollLower = chart.addLineSeries({ ...bollOpts, color: INDICATOR_COLORS.bollLower })

            if (showVolume) {
                seriesRef.current.volume = chart.addHistogramSeries({
                    priceFormat: { type: 'volume' },
                    priceScaleId: 'volume',
                })
                chart.priceScale('volume').applyOptions({
                    scaleMargins: { top: 0.85, bottom: 0 },
                    minimumWidth: 80,
                })
            }

            resizeObserver = new ResizeObserver(() => {
                if (containerRef.current && chartRef.current) {
                    chartRef.current.applyOptions({ width: containerRef.current.clientWidth })
                }
            })
            resizeObserver.observe(containerRef.current)

            if (sync && seriesRef.current.candle) {
                sync.register(chart, seriesRef.current.candle)
            }

            // ── 所有 series 建立完成，通知 Effect 2 可以執行 ──────────────────
            setChartReady(true)
        })

        return () => {
            hasAlignedRef.current = false
            isMounted = false
            resizeObserver?.disconnect()
            if (chartRef.current && sync) sync.unregister(chartRef.current)
            chartRef.current?.remove()
            chartRef.current = null
            setChartReady(false)
        }
    }, [height, colorMode, showVolume])


    // ── Effect 2：更新資料 ────────────────────────────────────────────────────
    // chartReady 在 deps 裡，確保 Effect 1 完成後才執行
    useEffect(() => {
        // Effect 1 還沒完成，等待
        if (!chartReady) return

        const s = seriesRef.current
        if (!s.candle || candles.length === 0) return

        const { up, down, upVolume, downVolume } = getCandleColors(colorMode)
        const show = (key: string) => !visibleIndicators || visibleIndicators.has(key)

        // Candlestick
        s.candle.setData(candles.map((c, idx) => {
            const prevClose = idx > 0 ? candles[idx - 1].close : c.open
            const isUp = c.close >= prevClose
            const color = isUp ? up : down
            return {
                time: (c.timestamp_ms / 1000) as Time,
                open: c.open, high: c.high, low: c.low, close: c.close,
                color, wickColor: color, borderColor: color,
            }
        }))


        // Markers
        if (signals.length > 0) {
            const firstMs = candles[0].timestamp_ms
            const lastMs = candles[candles.length - 1].timestamp_ms
            const markers = signals
                .filter((sig) => sig.timestamp_ms >= firstMs && sig.timestamp_ms <= lastMs)
                .sort((a, b) => a.timestamp_ms - b.timestamp_ms)
                .map((sig) => ({
                    time: (sig.timestamp_ms / 1000) as Time,
                    position: sig.signal_type === 'BUY' ? 'belowBar' as const : 'aboveBar' as const,
                    color: sig.signal_type === 'BUY' ? SIGNAL_TYPE.buy : SIGNAL_TYPE.sell,
                    shape: sig.signal_type === 'BUY' ? 'arrowUp' as const : 'arrowDown' as const,
                    text: markerTextMode === 'signalOnly'
                        ? sig.signal_type
                        : `${sig.signal_type} ${(sig.confidence * 100).toFixed(0)}%`,
                    size: sig.reliability === 'low' ? 1 : 2,
                }))
            s.candle.setMarkers(markers)
        } else {
            s.candle.setMarkers([])
        }

        // MA
        const toLineData = (key: string) =>
            candles
                .filter((c) => c.indicators?.[key] != null && !isNaN(c.indicators[key] as number))
                .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators[key] as number }))

        s.ma5?.setData(show('ma5') ? toLineData('ma5') : [])
        s.ma20?.setData(show('ma20') ? toLineData('ma20') : [])
        s.ma50?.setData(show('ma50') ? toLineData('ma50') : [])

        // Bollinger
        if (show('bollinger')) {
            const bollCandles = candles.filter((c) => c.indicators?.['bollinger'] != null)
            type Boll = { upper: number; middle: number; lower: number }
            s.bollUpper?.setData(bollCandles.map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: (c.indicators['bollinger'] as Boll).upper })))
            s.bollMid?.setData(bollCandles.map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: (c.indicators['bollinger'] as Boll).middle })))
            s.bollLower?.setData(bollCandles.map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: (c.indicators['bollinger'] as Boll).lower })))
        } else {
            s.bollUpper?.setData([])
            s.bollMid?.setData([])
            s.bollLower?.setData([])
        }

        // Volume
        s.volume?.setData(candles.map((c, idx) => {
            const prevClose = idx > 0 ? candles[idx - 1].close : c.open
            const isUp = c.close >= prevClose
            return {
                time: (c.timestamp_ms / 1000) as Time,
                value: c.volume,
                color: isUp ? upVolume : downVolume,
            }
        }))

        if (!hasAlignedRef.current && sync) {
            hasAlignedRef.current = true
            sync.alignRight()
        }

    }, [chartReady, candles, signals, visibleIndicators, colorMode, markerTextMode])

    return <div ref={containerRef} style={{ height }} className="w-full rounded-lg overflow-hidden" />
}

// ── IndicatorPane (RSI / MACD sub-charts) ─────────────────────────────────────

