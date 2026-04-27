import { useMutation, useQueryClient } from '@tanstack/react-query'
import { apiClient } from '@/src/lib/api-client'
import { generateRequestId } from '@/src/lib/utils'

import type { BacktestResponse, BacktestRequest } from '@/src/types/api.types'

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