
import { useQuery } from '@tanstack/react-query'
import { apiClient, buildQueryString } from '@/src/lib/api-client'
import { useFocusPolling } from './useFocusPolling'
import type {
    SignalsResponse,
} from '@/src/types/api.generated'

interface UseSignalsParams {
    symbol: string
    from_ms: number
    to_ms: number
}

export function useSignals(params: UseSignalsParams) {
    const refetchInterval = useFocusPolling()

    const qs = buildQueryString({ from_ms: params.from_ms, to_ms: params.to_ms })

    return useQuery<SignalsResponse>({
        queryKey: ['signals', params.symbol, params.from_ms, params.to_ms],
        queryFn: () => apiClient<SignalsResponse>(`/api/v1/signals/${params.symbol}${qs}`),
        staleTime: 25_000,
        gcTime: 5 * 60_000,
        refetchInterval,
        refetchIntervalInBackground: false,
        enabled: !!params.symbol,
    })
}