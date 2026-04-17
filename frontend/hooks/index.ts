/**
 * Custom hooks — encapsulate all React Query calls.
 * RULE: Components never call fetch() directly. All API access goes through these hooks.
 */
'use client'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiClient, buildQueryString } from '@/lib/api-client'
import { isMarketOpen, generateRequestId } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import type {
    SymbolsResponse,
    CandlesResponse,
    SignalsResponse,
    BacktestResponse,
    BacktestRequest,
} from '@/types/api.generated'

// ── useSymbols ────────────────────────────────────────────────────────────────

export function useSymbols() {
    return useQuery<SymbolsResponse>({
        queryKey: ['symbols'],
        queryFn: () => apiClient<SymbolsResponse>('/api/v1/symbols'),
        staleTime: 10 * 60 * 1000,   // 10 minutes
        gcTime: 30 * 60 * 1000,   // 30 minutes
    })
}

// ── useCandles ────────────────────────────────────────────────────────────────

interface UseCandlesParams {
    symbol: string
    interval: string
    from_ms: number
    to_ms: number
    indicators?: string   // comma-separated: 'ma20,rsi,macd'
    cursor?: string
}

export function useCandles(params: UseCandlesParams) {
    const isEco = useAppStore((s) => s.isEcoModeEnabled)
    const refetchInterval = isMarketOpen()
        ? 30_000
        : isEco ? 5 * 60_000 : 30_000

    const qs = buildQueryString({
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        interval: params.interval,
        indicators: params.indicators,
        cursor: params.cursor,
    })

    return useQuery<CandlesResponse>({
        queryKey: ['candles', params.symbol, params.interval, params.from_ms, params.to_ms],
        queryFn: () => apiClient<CandlesResponse>(`/api/v1/candles/${params.symbol}${qs}`),
        staleTime: 25_000,
        gcTime: 5 * 60_000,
        refetchInterval,
        enabled: !!params.symbol && !!params.from_ms && !!params.to_ms,
    })
}

// ── useSignals ────────────────────────────────────────────────────────────────

interface UseSignalsParams {
    symbol: string
    from_ms: number
    to_ms: number
}

export function useSignals(params: UseSignalsParams) {
    const isEco = useAppStore((s) => s.isEcoModeEnabled)
    const refetchInterval = isMarketOpen()
        ? 30_000
        : isEco ? 5 * 60_000 : 30_000

    const qs = buildQueryString({ from_ms: params.from_ms, to_ms: params.to_ms })

    return useQuery<SignalsResponse>({
        queryKey: ['signals', params.symbol, params.from_ms, params.to_ms],
        queryFn: () => apiClient<SignalsResponse>(`/api/v1/signals/${params.symbol}${qs}`),
        staleTime: 25_000,
        gcTime: 5 * 60_000,
        refetchInterval,
        enabled: !!params.symbol,
    })
}

// ── useBacktest ───────────────────────────────────────────────────────────────

export function useBacktest() {
    const queryClient = useQueryClient()

    return useMutation<BacktestResponse, Error, Omit<BacktestRequest, 'request_id'>>({
        mutationFn: (payload) =>
            apiClient<BacktestResponse>('/api/v1/backtest', {
                method: 'POST',
                body: JSON.stringify({ ...payload, request_id: generateRequestId() }),
            }),
        onSuccess: (data) => {
            queryClient.setQueryData(['backtest', data.backtest_id], data)
        },
    })
}