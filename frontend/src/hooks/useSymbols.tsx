import { useQuery } from '@tanstack/react-query'
import { apiClient, buildQueryString } from '@/src/lib/api-client'
import type { SymbolsResponse } from '@/src/types/api.types'

interface UseSymbolsOptions {
    includeInactive?: boolean
}

export function useSymbols(options: UseSymbolsOptions = {}) {
    const { includeInactive = false } = options

    // 只撈 active（預設行為，ManualSyncPanel 等其他地方不受影響）
    const activeQuery = useQuery<SymbolsResponse>({
        queryKey: ['symbols', { active: true }],
        queryFn: () => apiClient<SymbolsResponse>('/api/v1/symbols'),
        staleTime: 10 * 60 * 1000,
        gcTime: 30 * 60 * 1000,
    })

    // 只在 includeInactive 時才撈下市股票
    const inactiveQuery = useQuery<SymbolsResponse>({
        queryKey: ['symbols', { active: false }],
        queryFn: () => apiClient<SymbolsResponse>(
            `/api/v1/symbols${buildQueryString({ is_active: false })}`
        ),
        staleTime: 10 * 60 * 1000,
        gcTime: 30 * 60 * 1000,
        enabled: includeInactive,
    })

    if (!includeInactive) {
        return activeQuery
    }

    // 合併兩個 query 結果
    const isLoading = activeQuery.isLoading || inactiveQuery.isLoading
    const isError = activeQuery.isError || inactiveQuery.isError
    const error = activeQuery.error ?? inactiveQuery.error

    const mergedData: SymbolsResponse | undefined =
        activeQuery.data
            ? {
                symbols: [
                    ...(activeQuery.data.symbols ?? []),
                    ...(inactiveQuery.data?.symbols ?? []),
                ],
                count:
                    (activeQuery.data.count ?? 0) +
                    (inactiveQuery.data?.count ?? 0),
                last_synced_ms: activeQuery.data.last_synced_ms,
            }
            : undefined

    return {
        data: mergedData,
        isLoading,
        isError,
        error,
        refetch: () => {
            activeQuery.refetch()
            inactiveQuery.refetch()
        },
    }
}