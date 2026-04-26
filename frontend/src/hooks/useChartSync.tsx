'use client'
import { useRef, useCallback } from 'react'
import type { IChartApi, ISeriesApi, SeriesType } from 'lightweight-charts'

// ── CrosshairData：tooltip 所需的完整資料結構 ─────────────────────────────────

export interface CrosshairData {
    timestamp_ms: number | null
    open: number | null
    high: number | null
    low: number | null
    close: number | null
    volume: number | null
    indicators: Record<string, unknown>
}

// ── ChartSyncHandle ───────────────────────────────────────────────────────────

export interface ChartSyncHandle {
    register: (chart: IChartApi, series: ISeriesApi<SeriesType>) => void
    unregister: (chart: IChartApi) => void
    // Subscribe crosshair 資料，回傳 unsubscribe fn
    subscribeCrosshairData: (cb: (data: CrosshairData | null) => void) => () => void
}

export function useChartSync(): ChartSyncHandle {
    const chartsRef = useRef<Map<IChartApi, ISeriesApi<SeriesType>>>(new Map())
    const seriesRef = useRef<Map<IChartApi, ISeriesApi<SeriesType>>>(new Map())
    const maxLogicalRef = useRef<Map<IChartApi, number>>(new Map())
    const isSyncingRef = useRef(false)
    const dataLengthRef = useRef(0)

    // ── CrosshairData 訂閱清單 ────────────────────────────────────────────────
    const crosshairListeners = useRef<Set<(data: CrosshairData | null) => void>>(new Set())

    const subscribeCrosshairData = useCallback((cb: (data: CrosshairData | null) => void) => {
        crosshairListeners.current.add(cb)
        return () => { crosshairListeners.current.delete(cb) }
    }, [])

    const broadcastCrosshair = useCallback((data: CrosshairData | null) => {
        crosshairListeners.current.forEach(cb => cb(data))
    }, [])

    // ── 全域 master candle map（所有 chart 共用同一份）────────────────────────
    // key: timeSec（timestamp_ms / 1000），value: { open, high, low, close, volume, indicators }
    // 只有 CandleChart 的 feedCandleMap 會寫入；RSI/MACD chart 觸發 crosshair 時同樣能查到。
    const masterCandleMap = useRef<Map<number, Record<string, unknown>>>(new Map())

    const register = useCallback((chart: IChartApi, series: ISeriesApi<SeriesType>) => {
        chartsRef.current.set(chart, series)
        seriesRef.current.set(chart, series)

        // ── 同步 timeScale ─────────────────────────────────────────────────
        chart.timeScale().subscribeVisibleLogicalRangeChange((range) => {
            if (!range || isSyncingRef.current) return

            isSyncingRef.current = true
            chartsRef.current.forEach((_, other) => {
                if (other === chart) return
                other.timeScale().setVisibleLogicalRange(range)
            })
            setTimeout(() => { isSyncingRef.current = false }, 0)
        })

        // ── 同步 crosshair + 廣播 CrosshairData ───────────────────────────
        chart.subscribeCrosshairMove((param) => {
            if (!param.point || !param.time) {
                broadcastCrosshair(null)
            } else {
                const timeSec = param.time as number
                // 不管是哪個子圖觸發（K線 / RSI / MACD），
                // 一律從 masterCandleMap 查完整的 OHLCV + indicators。
                const entry = masterCandleMap.current.get(timeSec)

                broadcastCrosshair({
                    timestamp_ms: timeSec * 1000,
                    open: (entry?.['open'] as number) ?? null,
                    high: (entry?.['high'] as number) ?? null,
                    low: (entry?.['low'] as number) ?? null,
                    close: (entry?.['close'] as number) ?? null,
                    volume: (entry?.['volume'] as number) ?? null,
                    indicators: (entry?.['indicators'] as Record<string, unknown>) ?? {},
                })
            }

            // 同步其他圖表的 crosshair 位置
            chartsRef.current.forEach((otherSeries, other) => {
                if (other === chart) return

                if (!param.point || !param.time) {
                    other.clearCrosshairPosition()
                    return
                }

                const data = otherSeries.data() as any[]
                const dataPoint = data.find((d) => d.time === param.time)
                const price = dataPoint?.value ?? dataPoint?.close

                if (price === undefined || price === null || Number.isNaN(price)) {
                    other.clearCrosshairPosition()
                    return
                }

                other.setCrosshairPosition(price, param.time, otherSeries)
            })
        })
    }, [broadcastCrosshair])

    const unregister = useCallback((chart: IChartApi) => {
        chartsRef.current.delete(chart)
        seriesRef.current.delete(chart)
        maxLogicalRef.current.delete(chart)
    }, [])

    /**
     * CandleChart 在 setData 後呼叫此方法，把完整 candle 陣列（含 OHLCV、indicators）
     * 寫入 masterCandleMap。RSI/MACD chart 觸發 crosshair 時即可查到同一份資料。
     */
    const feedCandleMap = useCallback((candles: Array<{
        timestamp_ms: number
        open: number
        high: number
        low: number
        close: number
        volume?: number
        indicators?: Record<string, unknown>
    }>) => {
        const map = new Map<number, Record<string, unknown>>()
        candles.forEach((c) => {
            map.set(Math.floor(c.timestamp_ms / 1000), {
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume: c.volume,
                indicators: c.indicators ?? {},
            })
        })
        masterCandleMap.current = map
        dataLengthRef.current = candles.length
    }, [])

    return { register, unregister, subscribeCrosshairData, feedCandleMap } as ChartSyncHandle & {
        feedCandleMap: typeof feedCandleMap
    }
}