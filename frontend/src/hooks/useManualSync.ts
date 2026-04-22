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
import { apiClient, ApiErrorException } from '@/src/lib/api-client'
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
const API_V1_PREFIX = '/api/v1'

function buildAdminSyncPath(path: string): string {
  const base = (process.env.NEXT_PUBLIC_API_BASE_URL ?? '').replace(/\/+$/, '')
  const normalizedPath = path.startsWith('/') ? path : `/${path}`

  // 若 base 已包含 /api/v1，就不要再重複加前綴
  const prefix = base.endsWith(API_V1_PREFIX) ? '' : API_V1_PREFIX
  return `${prefix}${normalizedPath}`
}

/** status 是否仍在進行中（需要繼續輪詢） */
function isInProgress(status: SyncStatus): boolean {
  return status === 'running' || status === 'rate_limit_waiting'
}

// ── useTriggerSync ────────────────────────────────────────────────────────────

export function useTriggerSync() {
  const setActiveSyncId = useAppStore((s) => s.setActiveSyncId)
  const queryClient = useQueryClient()

  function extractConflictSyncId(error: Error): string | null {
    if (!(error instanceof ApiErrorException)) return null
    if (error.httpStatus !== 409) return null
    if (error.errorCode !== 'SYNC_ALREADY_RUNNING') return null

    const raw = error.apiError as unknown as { sync_id?: unknown }
    return typeof raw.sync_id === 'string' && raw.sync_id.length > 0 ? raw.sync_id : null
  }

  return useMutation<
    ManualSyncAcceptedResponse,
    Error,
    {
      symbols: string[]
      fullSync: boolean
      fromDate?: string
      toDate?: string
      intervals?: string[]
    }
  >({
    mutationFn: ({ symbols, fullSync, fromDate, toDate, intervals }) =>
      apiClient<ManualSyncAcceptedResponse>(buildAdminSyncPath('/admin/sync'), {
        method: 'POST',
        body: JSON.stringify({
          request_id: generateRequestId(),
          symbols,
          full_sync: fullSync,
          from_date: fullSync ? undefined : fromDate,
          to_date: fullSync ? undefined : toDate,
          intervals: fullSync ? undefined : intervals,
        } satisfies ManualSyncRequest),
      }),

    onSuccess: (data) => {
      // 將 sync_id 存入 store，供 useSyncStatus 開始輪詢
      setActiveSyncId(data.sync_id)
      // 預填快取，避免第一次輪詢前出現 loading 狀態
      queryClient.setQueryData(['sync-status', data.sync_id], {
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
    onError: (error) => {
      // 409: 已有同步進行中 → 接手既有 sync_id，直接切到進度輪詢
      const syncId = extractConflictSyncId(error)
      if (!syncId) return

      setActiveSyncId(syncId)
      queryClient.invalidateQueries({ queryKey: ['sync-status', syncId] })
    },
  })
}

export function useCancelSync() {
  const setActiveSyncId = useAppStore((s) => s.setActiveSyncId)

  return useMutation<void, Error, { syncId: string }>({
    mutationFn: async ({ syncId }) => {
      await apiClient(buildAdminSyncPath(`/admin/sync/cancel/${syncId}`), {
        method: 'POST',
      })
    },
    onSuccess: () => {
      setActiveSyncId(null)
    },
  })
}

// ── useSyncStatus ─────────────────────────────────────────────────────────────

export function useSyncStatus() {
  const syncId = useAppStore((s) => s.activeSyncId)
  const setActiveSyncId = useAppStore((s) => s.setActiveSyncId)

  const query = useQuery<SyncStatusResponse>({
    queryKey: ['sync-status', syncId],
    queryFn: async () => {
      if (!syncId) {
        throw new Error('Missing sync id')
      }
      try {
        return await apiClient<SyncStatusResponse>(
          buildAdminSyncPath(`/admin/sync/status/${syncId}`),
        )
      } catch (error) {
        // 只有真正查無此 sync_id 時才清空；避免短暫錯誤中斷追蹤
        if (error instanceof ApiErrorException && error.httpStatus === 404) {
          setActiveSyncId(null)
        }
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

  // 當 status 變為 completed / failed，清除 activeSyncId
  // 讓下次按「再次同步」時可以重新開始
  const status = query.data?.status
  /*
  if (status && !isInProgress(status) && syncId !== null) {
    // 延遲清除，確保最終狀態能被元件讀取到
    setTimeout(() => setActiveSyncId(null), 3000)
  }
  */

  return query
}
