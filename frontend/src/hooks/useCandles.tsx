import { useQuery } from '@tanstack/react-query'
import { apiClient, buildQueryString } from '@/src/lib/api-client'

import { useFocusPolling } from './useFocusPolling'
import type { CandlesResponse } from '@/src/types/api.types'

interface UseCandlesParams {
    symbol: string
    interval: string
    from_ms: number
    to_ms: number
    indicators?: string   // comma-separated: 'ma20,rsi,macd'
    cursor?: string
}

export function useCandles(params: UseCandlesParams) {
    const refetchInterval = useFocusPolling()

    const qs = buildQueryString({
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        interval: params.interval,
        indicators: params.indicators,
        cursor: params.cursor,
    })

    return useQuery<CandlesResponse>({
        queryKey: ['candles', params.symbol, params.interval, params.from_ms, params.to_ms, params.indicators],
        queryFn: () => apiClient<CandlesResponse>(`/api/v1/candles/${params.symbol}${qs}`),
        staleTime: 25_000,
        gcTime: 5 * 60_000,
        refetchInterval,
        refetchIntervalInBackground: false,
        enabled: !!params.symbol && !!params.from_ms && !!params.to_ms,
    })
}