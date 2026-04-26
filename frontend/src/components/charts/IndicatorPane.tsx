'use client'
import { useEffect, useRef } from 'react'
import type { CandleItem } from '@/src/types/api.generated'
import { Time } from 'lightweight-charts';
import { INDICATOR_COLORS } from '@/src/constants/chartColors'
import { ChartSyncHandle } from '@/src/hooks/useChartSync'
import { RsiTooltip, MacdTooltip } from './ChartTooltip'

interface IndicatorPaneProps {
    candles: CandleItem[]
    type: 'rsi14' | 'macd'
    sync?: ChartSyncHandle
}

export function IndicatorPane({ candles, type, sync }: IndicatorPaneProps) {
    const containerRef = useRef<HTMLDivElement>(null)
    const height = type === 'rsi14' ? 160 : 180

    useEffect(() => {
        if (!containerRef.current || candles.length === 0) return
        let chart: ReturnType<typeof import('lightweight-charts')['createChart']> | null = null
        let resizeObserver: ResizeObserver | null = null

        import('lightweight-charts').then(({ createChart }) => {
            if (!containerRef.current) return

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
                const macdLine = chart.addLineSeries({ color: INDICATOR_COLORS.macdLine, lineWidth: 1, priceLineVisible: false })
                const signalLine = chart.addLineSeries({ color: INDICATOR_COLORS.signal, lineWidth: 1, priceLineVisible: false })
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
                            color: m.histogram >= 0 ? INDICATOR_COLORS.histPos : INDICATOR_COLORS.histNeg
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
            resizeObserver?.disconnect()
            if (chart && sync) sync.unregister(chart)
            chart?.remove()
        }
    }, [candles, type, height])

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
                    ? <RsiTooltip sync={sync} />
                    : <MacdTooltip sync={sync} />
                }
            </div>
        </div>
    )
}