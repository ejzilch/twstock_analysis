'use client'
import { useRef, useCallback } from 'react'
import type { IChartApi, ISeriesApi, SeriesType } from 'lightweight-charts'

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
            if (!range || isSyncingRef.current) return;

            isSyncingRef.current = true;
            chartsRef.current.forEach((_, other) => {
                if (other === chart) return;
                other.timeScale().setVisibleLogicalRange(range);
            });
            setTimeout(() => { isSyncingRef.current = false; }, 0);
        });

        // ── 同步 crosshair ─────────────────────────────────────────────────
        chart.subscribeCrosshairMove((param) => {
            chartsRef.current.forEach((otherSeries, other) => {
                if (other === chart) return;

                // 防護：滑鼠移出圖表或無效時間
                if (!param.point || !param.time) {
                    other.clearCrosshairPosition();
                    return;
                }

                // 取得副圖資料，並直接用 param.time 尋找「同一天」的資料點
                // 這比使用 logical index 更安全，能徹底避免陣列長度偏移問題
                const data = otherSeries.data() as any[];
                const dataPoint = data.find((d) => d.time === param.time);

                // 取得該點數值 (指標用 value，K線用 close)
                const price = dataPoint?.value ?? dataPoint?.close;

                // 嚴格排除 undefined, null 以及 NaN
                if (price === undefined || price === null || Number.isNaN(price)) {
                    // 如果這天是空白資料，徹底清除游標 (隱藏垂直與水平線)
                    other.clearCrosshairPosition();
                    return;
                }

                // 確保 price 是正常的數字後，畫出精準的十字線
                other.setCrosshairPosition(price, param.time, otherSeries);
            });
        });
    }, [])

    const unregister = useCallback((chart: IChartApi) => {
        chartsRef.current.delete(chart)
        seriesRef.current.delete(chart)
        maxLogicalRef.current.delete(chart)
    }, [])

    const alignRight = useCallback((defaultBarsVisible = 88) => {
        requestAnimationFrame(() => {
            seriesRef.current.forEach((series, chart) => {
                const dataLen = series.data().length
                if (dataLen > 0) {
                    maxLogicalRef.current.set(chart, dataLen - 1)
                }
            })

            isSyncingRef.current = true
            chartsRef.current.forEach((_, chart) => {
                const max = maxLogicalRef.current.get(chart)
                if (max == null) return

                // 先讓圖表自己貼右，讀取它實際用的 rightOffset
                chart.timeScale().scrollToPosition(0, false)
            })
            isSyncingRef.current = false

            // 再下一幀讀取實際 offset 後統一設定 visible range
            requestAnimationFrame(() => {
                isSyncingRef.current = true
                chartsRef.current.forEach((_, chart) => {
                    const max = maxLogicalRef.current.get(chart)
                    if (max == null) return
                    const range = chart.timeScale().getVisibleLogicalRange()
                    // scrollToPosition(0) 後，range.to 就是圖表自己算的「貼右」位置
                    const actualTo = range?.to ?? max
                    chart.timeScale().setVisibleLogicalRange({
                        from: actualTo - defaultBarsVisible,
                        to: actualTo,
                    })
                })
                isSyncingRef.current = false
            })
        })
    }, [])

    return { register, unregister, alignRight }
}