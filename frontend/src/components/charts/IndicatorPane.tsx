'use client'
import { useEffect, useRef } from 'react'
import type { CandleItem } from '@/src/types/api.generated'
import { Time } from 'lightweight-charts';
import { INDICATOR_COLORS, ColorMode, getMacdRsiColors } from '@/src/constants/chartColors'
import { ChartSyncHandle } from '@/src/hooks/useChartSync'
import { RsiTooltip, MacdTooltip } from './ChartTooltip'
import { useAppStore } from '@/src/store/useAppStore'

interface IndicatorPaneProps {
    candles: CandleItem[]
    type: 'rsi14' | 'macd'
    sync?: ChartSyncHandle
    colorMode?: ColorMode
}

export function IndicatorPane({ candles, type, sync }: IndicatorPaneProps) {
    const colorMode = useAppStore((s) => s.colorMode)
    const containerRef = useRef<HTMLDivElement>(null)
    const chartRef = useRef<any>(null)
    const height = type === 'rsi14' ? 160 : 180
    const mc = getMacdRsiColors(colorMode ?? 'TW')

    useEffect(() => {
        if (!containerRef.current || candles.length === 0) return
        let chart: ReturnType<typeof import('lightweight-charts')['createChart']> | null = null
        let resizeObserver: ResizeObserver | null = null
        let isCancelled = false

        import('lightweight-charts').then(({ createChart }) => {
            if (isCancelled || !containerRef.current) return

            chartRef.current?.remove()
            chartRef.current = null

            chart = createChart(containerRef.current, {
                width: containerRef.current.clientWidth,
                height,
                layout: { background: { color: '#161b27' }, textColor: '#94a3b8' },
                grid: { vertLines: { color: '#1e2a3a' }, horzLines: { color: '#1e2a3a' } },
                rightPriceScale: {
                    borderColor: '#1e2a3a',
                    minimumWidth: 80,
                },
                timeScale: {
                    borderColor: '#1e2a3a',
                    timeVisible: true,
                    secondsVisible: false,
                    rightOffset: 0,
                },
            })

            chartRef.current = chart

            if (type === 'rsi14') {
                const rsiSeries = chart.addLineSeries({ color: INDICATOR_COLORS.rsi, lineWidth: 1, priceLineVisible: false })
                rsiSeries.setData(candles.map((c) => {
                    const time = (c.timestamp_ms / 1000) as Time;
                    const rsiValue = c.indicators?.['rsi14'];
                    if (rsiValue == null || isNaN(rsiValue as number)) return { time };
                    return { time, value: rsiValue as number };
                }));
                if (sync) sync.register(chart, rsiSeries)
            } else {
                const macdLine = chart.addLineSeries({ color: mc.macdLine, lineWidth: 1, priceLineVisible: false })
                const signalLine = chart.addLineSeries({ color: mc.signal, lineWidth: 1, priceLineVisible: false })
                const histogram = chart.addHistogramSeries({ priceLineVisible: false })

                const macdData: any[] = []
                const signalData: any[] = []
                const histData: any[] = []

                candles.forEach((c) => {
                    const t = (c.timestamp_ms / 1000) as Time
                    const m = c.indicators?.['macd'] as { macd_line: number; signal_line: number; histogram: number } | undefined

                    if (m == null || isNaN(m.macd_line)) {
                        macdData.push({ time: t })
                        signalData.push({ time: t })
                        histData.push({ time: t })
                    } else {
                        macdData.push({ time: t, value: m.macd_line })
                        signalData.push({ time: t, value: m.signal_line })
                        histData.push({
                            time: t,
                            value: m.histogram,
                            color: m.histogram >= 0 ? mc.histPos : mc.histNeg
                        })
                    }
                })

                macdLine.setData(macdData)
                signalLine.setData(signalData)
                histogram.setData(histData)

                if (sync) sync.register(chart, macdLine)
            }

            resizeObserver = new ResizeObserver(() => {
                if (containerRef.current && chart) {
                    chart.applyOptions({ width: containerRef.current.clientWidth })
                }
            })
            resizeObserver.observe(containerRef.current)
        })

        return () => {
            isCancelled = true
            resizeObserver?.disconnect()
            if (chart && sync) sync.unregister(chart)
            chartRef.current?.remove()
            chartRef.current = null
        }
    }, [candles, type, height, colorMode])

    return (
        <div className="w-full">
            {/* Label row */}
            <div className="px-3 py-1 text-xs text-slate-500 uppercase tracking-wider font-medium">
                {type === 'rsi14'
                    ? 'RSI (14) 超賣 < 30% , 超買 > 70%'
                    : 'MACD (12,26,9) 紅：快線 (MACD Line / DIF) , 綠：慢線 (Signal Line / DEM) , 柱狀圖 (Histogram / OSC)'}
            </div>

            {/* Chart area — relative so the tooltip anchors inside it */}
            <div className="relative">
                <div ref={containerRef} style={{ height }} className="w-full" />

                {/* Fixed top-left overlay tooltip */}
                {type === 'rsi14'
                    ? <RsiTooltip sync={sync} mc={mc} />
                    : <MacdTooltip sync={sync} mc={mc} />
                }
            </div>
        </div>
    )
}