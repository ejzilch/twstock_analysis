/**
 * Business layer derived types.
 * Composed from api.generated.ts via Pick<>/Omit<> — never duplicate interface definitions.
 */
import type { components } from './api.generated'

type Schemas = components['schemas']

export type CandleItem = Schemas['CandleItem']
export type CandlesResponse = Schemas['CandlesResponse']

export type SignalItem = Schemas['Signal']
export type SignalsResponse = Schemas['SignalsResponse']

export type SymbolItem = Schemas['Symbol']
export type SymbolsResponse = Schemas['SymbolsResponse']

export type BacktestRequest = Schemas['BacktestRequest']
export type BacktestResponse = Schemas['BacktestResponse']

// ── Enums ─────────────────────────────────────────────────────────────────────
export type Interval = '1m' | '5m' | '15m' | '1h' | '4h' | '1d'
export type Exchange = 'TWSE' | 'TPEX'
export type SignalType = 'BUY' | 'SELL'
export type SignalSource = 'ai_ensemble' | 'technical_only' | 'manual_override'
export type ReliabilityLevel = 'high' | 'medium' | 'low' | 'unknown'
export type HealthStatus = 'ok' | 'degraded' | 'unavailable'
export type DataLatencyStatus = 'ok' | 'warning' | 'critical'

// Candle with guaranteed indicator shapes for chart rendering
export type CandleWithIndicators = CandleItem & {
    indicators: {
        ma5?: number
        ma20?: number
        ma50?: number
        rsi?: number
        macd?: MacdValue
        bollinger?: BollingerValue
    }
}

// ── Indicator Types ───────────────────────────────────────────────────────────
export type IndicatorValue = number | MacdValue | BollingerValue

export interface MacdValue {
    macd_line: number
    signal_line: number
    histogram: number
}

export interface BollingerValue {
    upper: number
    middle: number
    lower: number
}

// ── Manual Sync ───────────────────────────────────────────────────────────────

export type SyncStatus =
    | 'running'
    | 'rate_limit_waiting'
    | 'completed'
    | 'failed'

export type SymbolSyncStatus =
    | 'pending'
    | 'running'
    | 'completed'
    | 'failed'
    | 'skipped'

export interface GapProgress {
    from_ms: number
    to_ms: number
    inserted: number
    skipped: number
    failed: number
    completed: boolean
}

export interface SymbolProgress {
    symbol: string
    name: string
    status: SymbolSyncStatus
    gap_a: GapProgress | null
    gap_b: GapProgress | null
}

export interface RateLimitInfo {
    used_this_hour: number
    limit_per_hour: number
    is_waiting: boolean
    /** rate limit 解除的毫秒級 timestamp，is_waiting === false 時為 null */
    resume_at_ms: number | null
}

export interface SyncSummary {
    total_symbols: number
    completed_symbols: number
    total_inserted: number
    total_skipped: number
    total_failed: number
}

/** POST /api/v1/admin/sync Request */
export interface ManualSyncRequest {
    request_id: string
    mode: string
    symbols: string[] | undefined
    full_sync: boolean
    from_date: string | undefined
    to_date: string | undefined
    intervals: string[] | undefined
}

/** POST /api/v1/admin/sync Response 202 */
export interface ManualSyncAcceptedResponse {
    sync_id: string
    status: SyncStatus
    symbols: string[]
    estimated_requests: number
    estimated_hours: number
    started_at_ms: number
}

/** GET /api/v1/admin/sync/status Response 200 */
export interface SyncStatusResponse {
    sync_id: string
    status: SyncStatus
    started_at_ms: number
    rate_limit: RateLimitInfo
    progress: SymbolProgress[]
    summary: SyncSummary
}

// ── Errors ────────────────────────────────────────────────────────────────────

export type ErrorCode =
    | 'UNAUTHORIZED'
    | 'AI_SERVICE_TIMEOUT'
    | 'AI_SERVICE_UNAVAILABLE'
    | 'DATA_SOURCE_INTERRUPTED'
    | 'DATA_SOURCE_RATE_LIMITED'
    | 'INDICATOR_COMPUTE_FAILED'
    | 'CACHE_MISS_FALLBACK'
    | 'COMPUTATION_OVERFLOW'
    | 'INVALID_INDICATOR_CONFIG'
    | 'SYMBOL_NOT_FOUND'
    | 'QUERY_RANGE_TOO_LARGE'
    | 'SYNC_ALREADY_RUNNING'
    | 'SYNC_NOT_FOUND'
    | 'FINMIND_UNAVAILABLE'

export interface ApiError {
    error_code: ErrorCode
    message: string
    fallback_available: boolean
    timestamp_ms: number
    request_id: string | null
}

// Lightweight symbol for selector UI
export type SymbolOption = Pick<SymbolItem, 'symbol' | 'name' | 'exchange'>

// Signal enriched with display metadata
export type SignalWithDisplay = SignalItem & {
    displayLabel: string
    displayColor: string
}

// Badge configuration derived from reliability
export interface BadgeConfig {
    label: string
    color: 'green' | 'yellow' | 'gray' | 'red'
    bg: string
    text: string
}

export const RELIABILITY_BADGE: Record<ReliabilityLevel, BadgeConfig> = {
    high: { label: 'AI 高信心', color: 'green', bg: 'bg-emerald-500/15', text: 'text-emerald-400' },
    medium: { label: 'AI 中信心', color: 'yellow', bg: 'bg-amber-500/15', text: 'text-amber-400' },
    low: { label: '技術指標', color: 'gray', bg: 'bg-slate-500/15', text: 'text-slate-400' },
    unknown: { label: '信號異常', color: 'red', bg: 'bg-red-500/15', text: 'text-red-400' },
}

// Zustand store shape
export interface AppState {
    activeSyncId: string | null,
    selectedSymbol: string
    selectedInterval: string
    isEcoModeEnabled: boolean
    apiKey: string
    colorMode: 'TW' | 'US'
    setActiveSyncId: (id: string | null) => void
    toggleColorMode: () => void
    setSelectedSymbol: (symbol: string) => void
    setSelectedInterval: (interval: string) => void
    toggleEcoMode: () => void
    setApiKey: (key: string) => void
}