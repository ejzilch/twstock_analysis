'use client'
/**
 * src/hooks/useManualSync.ts
 *
 * 手動同步相關 hooks。
 *
 * useTriggerSync()  — mutation，觸發 POST /api/v1/admin/sync
 * useSyncStatus()   — query，輪詢 GET /api/v1/admin/sync/status
 *
 * 規則：
 *   - 僅在 syncId 存在且 status 為 running / rate_limit_waiting 時啟動輪詢
 *   - 背景 tab 不輪詢（refetchIntervalInBackground: false）
 *   - status 為 completed / failed 時自動停止輪詢
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from '@/src/lib/api-client'
import { generateRequestId } from '@/src/lib/utils'
import { useAppStore } from '@/src/store/useAppStore'
import type {
  ManualSyncAcceptedResponse,
  ManualSyncRequest,
  SyncStatus,
  SyncStatusResponse,
} from '@/src/types/api.generated'

// 輪詢間隔：10 秒
const SYNC_POLL_INTERVAL_MS = 10_000

/** status 是否仍在進行中（需要繼續輪詢） */
function isInProgress(status: SyncStatus): boolean {
  return status === 'running' || status === 'rate_limit_waiting'
}

// ── useTriggerSync ────────────────────────────────────────────────────────────

export function useTriggerSync() {
  const setActiveSyncId = useAppStore((s) => s.setActiveSyncId)
  const queryClient = useQueryClient()

  return useMutation<
    ManualSyncAcceptedResponse,
    Error,
    { symbols: string[] }
  >({
    mutationFn: ({ symbols }) =>
      apiClient<ManualSyncAcceptedResponse>('/api/v1/admin/sync', {
        method: 'POST',
        body: JSON.stringify({
          request_id: generateRequestId(),
          symbols,
        } satisfies ManualSyncRequest),
      }),

    onSuccess: (data) => {
      // 將 sync_id 存入 store，供 useSyncStatus 開始輪詢
      setActiveSyncId(data.sync_id)
      // 預填快取，避免第一次輪詢前出現 loading 狀態
      queryClient.setQueryData(['sync-status'], {
        sync_id: data.sync_id,
        status: data.status,
        started_at_ms: data.started_at_ms,
        rate_limit: {
          used_this_hour: 0,
          limit_per_hour: 600,
          is_waiting: false,
          resume_at_ms: null,
        },
        progress: data.symbols.map((s) => ({
          symbol: s,
          name: '',
          status: 'pending' as const,
          gap_a: null,
          gap_b: null,
        })),
        summary: {
          total_symbols: data.symbols.length,
          completed_symbols: 0,
          total_inserted: 0,
          total_skipped: 0,
          total_failed: 0,
        },
      } satisfies SyncStatusResponse)
    },
  })
}

// ── useSyncStatus ─────────────────────────────────────────────────────────────

export function useSyncStatus() {
  const syncId = useAppStore((s) => s.activeSyncId)
  const setActiveSyncId = useAppStore((s) => s.setActiveSyncId)

  const query = useQuery<SyncStatusResponse>({
    queryKey: ['sync-status'],
    queryFn: async () => {
      try {
        return await apiClient<SyncStatusResponse>('/api/v1/admin/sync/status')
      } catch (error) {
        // API 404 代表 sync 不存在，清除殘留 syncId
        setActiveSyncId(null)
        throw error
      }
    },

    // 只在 syncId 存在時啟動
    enabled: syncId !== null,

    // 輪詢：只在進行中時觸發
    refetchInterval: (query) => {
      const status = query.state.data?.status
      if (!status || !isInProgress(status)) return false
      return SYNC_POLL_INTERVAL_MS
    },

    refetchIntervalInBackground: false,
    staleTime: 0,  // 同步狀態永遠視為過期，確保每次輪詢都更新

    throwOnError: false,
    retry: false,
  })

  if (query.isError && syncId !== null) {
    setActiveSyncId(null)
  }

  // 當 status 變為 completed / failed，清除 activeSyncId
  // 讓下次按「再次同步」時可以重新開始
  const status = query.data?.status
  if (status && !isInProgress(status) && syncId !== null) {
    // 延遲清除，確保最終狀態能被元件讀取到
    setTimeout(() => setActiveSyncId(null), 3000)
  }


  return query
}
