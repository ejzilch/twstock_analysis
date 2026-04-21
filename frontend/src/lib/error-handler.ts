/**
 * Maps API error_code to UI behaviours.
 * No React imports — pure logic consumed by hooks and components.
 */
import type { ErrorCode } from '@/src/types/api.generated'
import { ApiErrorException } from './api-client'

export type ToastVariant = 'info' | 'warning' | 'error' | 'silent'

export interface ErrorUIAction {
    variant: ToastVariant
    message: string
    redirect?: string     // client-side route to push
    showRetry?: boolean
    showInline?: boolean    // render near the triggering field
}

const ERROR_MAP: Record<ErrorCode, ErrorUIAction> = {
    UNAUTHORIZED: {
        variant: 'error',
        message: '請設定有效的 API Key',
        redirect: '/settings',
    },
    AI_SERVICE_TIMEOUT: {
        variant: 'warning',
        message: 'AI 算力繁忙，目前顯示技術指標信號',
    },
    AI_SERVICE_UNAVAILABLE: {
        variant: 'warning',
        message: 'AI 服務暫停，請稍後',
    },
    DATA_SOURCE_INTERRUPTED: {
        variant: 'error',
        message: '數據源暫中斷，顯示快取數據',
    },
    DATA_SOURCE_RATE_LIMITED: {
        variant: 'warning',
        message: '資料來源切換備援中，可能短暫延遲',
    },
    INDICATOR_COMPUTE_FAILED: {
        variant: 'error',
        message: '指標計算異常，請重新整理',
        showRetry: true,
    },
    COMPUTATION_OVERFLOW: {
        variant: 'error',
        message: '計算數值異常，請聯繫支援',
    },
    INVALID_INDICATOR_CONFIG: {
        variant: 'error',
        message: '指標設定錯誤',
        showInline: true,
    },
    SYMBOL_NOT_FOUND: {
        variant: 'error',
        message: '找不到此股票',
        showInline: true,
    },
    QUERY_RANGE_TOO_LARGE: {
        variant: 'error',
        message: '查詢範圍過大，請縮小時間區間或分批載入',
        showInline: true,
    },
    CACHE_MISS_FALLBACK: {
        variant: 'silent',
        message: '',
    },
    SYNC_ALREADY_RUNNING: {
        variant: 'warning',
        message: '同步執行中，請稍候',
    },
    SYNC_NOT_FOUND: {
        variant: 'silent',
        message: '',
    },
    FINMIND_UNAVAILABLE: {
        variant: 'error',
        message: 'FinMind 暫時無法連線，請稍後重試',
        showRetry: true,
    },
}

export function resolveErrorAction(error: unknown): ErrorUIAction {
    if (error instanceof ApiErrorException) {
        return ERROR_MAP[error.errorCode] ?? {
            variant: 'error',
            message: error.message,
        }
    }
    return {
        variant: 'error',
        message: error instanceof Error ? error.message : '發生未知錯誤',
    }
}

export function getErrorAction(code: ErrorCode): ErrorUIAction {
    return ERROR_MAP[code]
}