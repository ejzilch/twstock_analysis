'use client'
import { useRef, useCallback } from 'react'
import type { IChartApi, ISeriesApi, SeriesType, Logical } from 'lightweight-charts'

export interface ChartSyncHandle {
    register: (chart: IChartApi, series: ISeriesApi<SeriesType>) => void
    unregister: (chart: IChartApi) => void
    alignRight: (defaultBarsVisible?: number) => void
}

export function useChartSync(): ChartSyncHandle {
    const chartsRef = useRef<Map<IChartApi, ISeriesApi<SeriesType>>>(new Map())
    // key: chart, value: 該 chart 的 series（用來讀 dataLength）
    const seriesRef = useRef<Map<IChartApi, ISeriesApi<SeriesType>>>(new Map())
    const maxLogicalRef = useRef<Map<IChartApi, number>>(new Map())
    const isSyncingRef = useRef(false)

    /** 從 series 精確算出 maxLogical = dataLength - 1 */
    const calcMaxLogical = (series: ISeriesApi<SeriesType>): number => {
        return series.data().length - 1
    }

    const register = useCallback((chart: IChartApi, series: ISeriesApi<SeriesType>) => {
        chartsRef.current.set(chart, series)
        seriesRef.current.set(chart, series)

        // ── 同步 timeScale ─────────────────────────────────────────────────
        chart.timeScale().subscribeVisibleLogicalRangeChange((range) => {
            if (!range || isSyncingRef.current) return

            const myMax = maxLogicalRef.current.get(chart)
            if (myMax == null) return

            const rightOffset = range.to - myMax
            const barsVisible = range.to - range.from

            isSyncingRef.current = true
            chartsRef.current.forEach((_, other) => {
                if (other === chart) return
                const otherMax = maxLogicalRef.current.get(other)
                if (otherMax == null) return

                const clampedTo = Math.min(otherMax + rightOffset, otherMax + 20)
                const clampedFrom = clampedTo - barsVisible
                other.timeScale().setVisibleLogicalRange({ from: clampedFrom, to: clampedTo })
            })
            isSyncingRef.current = false
        })

        // ── 同步 crosshair ─────────────────────────────────────────────────
        chart.subscribeCrosshairMove((param) => {
            chartsRef.current.forEach((otherSeries, other) => {
                if (other === chart) return

                if (param.logical == null) {
                    other.clearCrosshairPosition()
                    return
                }

                const myMax = maxLogicalRef.current.get(chart)
                const otherMax = maxLogicalRef.current.get(other)
                if (myMax == null || otherMax == null) {
                    other.clearCrosshairPosition()
                    return
                }

                const offsetFromRight = param.logical - myMax
                const otherLogical = otherMax + offsetFromRight

                const coord = other.timeScale().logicalToCoordinate(otherLogical as Logical)
                if (coord == null) {
                    other.clearCrosshairPosition()
                    return
                }
                const time = other.timeScale().coordinateToTime(coord)
                if (time == null) {
                    other.clearCrosshairPosition()
                    return
                }

                other.setCrosshairPosition(0, time, otherSeries)
            })
        })
    }, [])

    const unregister = useCallback((chart: IChartApi) => {
        chartsRef.current.delete(chart)
        seriesRef.current.delete(chart)
        maxLogicalRef.current.delete(chart)
    }, [])

    const alignRight = useCallback((defaultBarsVisible = 60) => {
        // 從 series.data().length 精確算出 maxLogical，不依賴 getVisibleLogicalRange
        seriesRef.current.forEach((series, chart) => {
            const dataLen = series.data().length
            if (dataLen > 0) {
                maxLogicalRef.current.set(chart, dataLen - 1)
            }
        })

        // 所有 chart 設為「最後 defaultBarsVisible 根靠右」
        isSyncingRef.current = true
        chartsRef.current.forEach((_, chart) => {
            const max = maxLogicalRef.current.get(chart)
            if (max == null) return
            chart.timeScale().setVisibleLogicalRange({
                from: max - defaultBarsVisible,
                to: max,
            })
        })
        isSyncingRef.current = false
    }, [])

    return { register, unregister, alignRight }
}