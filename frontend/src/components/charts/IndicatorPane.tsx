'use client'
import { useEffect, useRef } from 'react'
import type { CandleItem } from '@/src/types/api.generated'
import { Time } from 'lightweight-charts';
import {
    INDICATOR_COLORS,
} from '@/src/constants/chartColors'
import { ChartSyncHandle } from '@/src/hooks/useChartSync'

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
                    rightOffset: 12,
                },
            })

            if (type === 'rsi14') {
                const rsiSeries = chart.addLineSeries({ color: INDICATOR_COLORS.rsi, lineWidth: 1, priceLineVisible: false })
                rsiSeries.setData(candles.map((c) => {
                    const time = (c.timestamp_ms / 1000) as Time;
                    const rsiValue = c.indicators?.['rsi14'];

                    // 如果沒有 RSI 數值 (例如前 14 根 K 線)
                    if (rsiValue == null || isNaN(rsiValue as number)) {
                        return { time }; // ✅ 只回傳 time，這在 Lightweight Charts 中代表「空白資料」
                    }

                    // 正常的資料點
                    return {
                        time,
                        value: rsiValue as number
                    };
                }));

                if (sync) {
                    sync.register(chart, rsiSeries)
                }
            } else {
                const macdLine = chart.addLineSeries({ color: INDICATOR_COLORS.macdLine, lineWidth: 1, priceLineVisible: false })
                const signalLine = chart.addLineSeries({ color: INDICATOR_COLORS.signal, lineWidth: 1, priceLineVisible: false })
                const histogram = chart.addHistogramSeries({ priceLineVisible: false })

                // 1. 準備三個陣列來裝整理好的資料
                const macdData: any[] = []
                const signalData: any[] = []
                const histData: any[] = []

                // 2. 拔掉 filter，遍歷每一根 K 線以確保長度 100% 一致
                candles.forEach((c) => {
                    const t = (c.timestamp_ms / 1000) as Time
                    const m = c.indicators?.['macd'] as { macd_line: number; signal_line: number; histogram: number } | undefined

                    if (m == null || isNaN(m.macd_line)) {
                        // 【關鍵】如果沒有資料，塞入「空白點」 (只有 time，沒有 value)
                        macdData.push({ time: t })
                        signalData.push({ time: t })
                        histData.push({ time: t })
                    } else {
                        // 有資料，正常塞入數值
                        macdData.push({ time: t, value: m.macd_line })
                        signalData.push({ time: t, value: m.signal_line })
                        histData.push({
                            time: t,
                            value: m.histogram,
                            color: m.histogram >= 0 ? INDICATOR_COLORS.histPos : INDICATOR_COLORS.histNeg
                        })
                    }
                })

                // 3. 一次性匯入圖表，效能最好
                macdLine.setData(macdData)
                signalLine.setData(signalData)
                histogram.setData(histData)

                if (sync) {
                    sync.register(chart, macdLine)
                }
            }
        })

        return () => {
            if (chart && sync) sync.unregister(chart)
            chart?.remove()
        }
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