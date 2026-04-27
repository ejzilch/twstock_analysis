
import { useQuery } from '@tanstack/react-query'
import { apiClient } from '@/src/lib/api-client'

import type { SymbolsResponse } from '@/src/types/api.types'

export function useSymbols() {
    return useQuery<SymbolsResponse>({
        queryKey: ['symbols'],
        queryFn: () => apiClient<SymbolsResponse>('/api/v1/symbols'),
        staleTime: 10 * 60 * 1000,   // 10 minutes
        gcTime: 30 * 60 * 1000,   // 30 minutes
    })
}