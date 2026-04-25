'use client'
import { useRef, useCallback } from 'react'
import type { IChartApi, ISeriesApi, SeriesType } from 'lightweight-charts'

export interface ChartSyncHandle {
    register: (chart: IChartApi, series: ISeriesApi<SeriesType>) => void
    unregister: (chart: IChartApi) => void
}

export function useChartSync(): ChartSyncHandle {
    // 對應的第一個 series（用來設 crosshair position）
    const chartsRef = useRef<Map<IChartApi, ISeriesApi<SeriesType>>>(new Map())

    const register = useCallback((chart: IChartApi, series: ISeriesApi<SeriesType>) => {
        const charts = chartsRef.current
        charts.set(chart, series)

        // ── 同步 timeScale ────────────────────────────────────────────────
        chart.timeScale().subscribeVisibleLogicalRangeChange((range) => {
            if (!range) return
            charts.forEach((_, other) => {
                if (other === chart) return
                other.timeScale().setVisibleLogicalRange(range)
            })
        })

        // ── 同步 crosshair ────────────────────────────────────────────────
        chart.subscribeCrosshairMove((param) => {
            charts.forEach((otherSeries, other) => {
                if (other === chart) return
                if (!param.time) {
                    other.clearCrosshairPosition()
                    return
                }
                // price 用 0，crosshair 水平線位置不重要，重要的是垂直時間線
                other.setCrosshairPosition(0, param.time, otherSeries)
            })
        })
    }, [])

    const unregister = useCallback((chart: IChartApi) => {
        chartsRef.current.delete(chart)
    }, [])

    return { register, unregister }
}