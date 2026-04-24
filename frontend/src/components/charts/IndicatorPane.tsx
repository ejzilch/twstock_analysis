'use client'
import { useEffect, useRef } from 'react'
import type { CandleItem } from '@/src/types/api.generated'
import { Time } from 'lightweight-charts';
import {
    INDICATOR_COLORS,
} from '@/src/constants/chartColors'

interface IndicatorPaneProps {
    candles: CandleItem[]
    type: 'rsi14' | 'macd'
}

export function IndicatorPane({ candles, type }: IndicatorPaneProps) {
    const containerRef = useRef<HTMLDivElement>(null)
    const height = type === 'rsi14' ? 100 : 120

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
                timeScale: { borderColor: '#1e2a3a', timeVisible: false, secondsVisible: false },
            })

            if (type === 'rsi14') {
                const rsiSeries = chart.addLineSeries({ color: INDICATOR_COLORS.rsi, lineWidth: 1, priceLineVisible: false })
                rsiSeries.setData(candles
                    .filter((c) => c.indicators?.['rsi14'] != null)
                    .map((c) => ({ time: (c.timestamp_ms / 1000) as Time, value: c.indicators['rsi14'] as number })))
            } else {
                const macdLine = chart.addLineSeries({ color: INDICATOR_COLORS.macdLine, lineWidth: 1, priceLineVisible: false })
                const signalLine = chart.addLineSeries({ color: INDICATOR_COLORS.signal, lineWidth: 1, priceLineVisible: false })
                const histogram = chart.addHistogramSeries({ priceLineVisible: false })

                candles.filter((c) => c.indicators?.['macd'] != null).forEach((c) => {
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
                {type === 'rsi14' ? 'RSI (14) 超賣 < 30% , 超買 > 70%'
                    : 'MACD (12,26,9) 紅：快線 (MACD Line / DIF) , 綠：慢線 (Signal Line / DEM) , 柱狀圖 (Histogram / OSC)'}
            </div>
            <div ref={containerRef} style={{ height }} className="w-full" />
        </div>
    )
}