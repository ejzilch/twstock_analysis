/**
 * AUTO-GENERATED FILE - DO NOT EDIT MANUALLY
 * Generated from OpenAPI Spec via: npx openapi-typescript ./docs/openapi.yaml -o src/types/api.generated.ts
 * Source of truth: API_CONTRACT.md v2.2
 */

// ── Enums ─────────────────────────────────────────────────────────────────────

export type Interval = '1m' | '5m' | '15m' | '1h' | '4h' | '1d'
export type Exchange = 'TWSE' | 'TPEX'
export type SignalType = 'BUY' | 'SELL'
export type SignalSource = 'ai_ensemble' | 'technical_only' | 'manual_override'
export type ReliabilityLevel = 'high' | 'medium' | 'low' | 'unknown'
export type HealthStatus = 'ok' | 'degraded' | 'unavailable'
export type DataLatencyStatus = 'ok' | 'warning' | 'critical'

// ── Indicator Types ───────────────────────────────────────────────────────────

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

export type IndicatorValue = number | MacdValue | BollingerValue

// ── Symbol ────────────────────────────────────────────────────────────────────

export interface SymbolItem {
    symbol: string
    name: string
    exchange: Exchange
    data_source: string
    earliest_available_ms: number
    latest_available_ms: number
    is_active: boolean
}

export interface SymbolsResponse {
    symbols: SymbolItem[]
    count: number
    last_synced_ms: number
}

// ── Candles ───────────────────────────────────────────────────────────────────

export interface CandleItem {
    timestamp_ms: number
    open: number
    high: number
    low: number
    close: number
    volume: number
    indicators: Record<string, IndicatorValue>
}

export interface CandlesResponse {
    symbol: string
    interval: Interval
    from_ms: number
    to_ms: number
    candles: CandleItem[]
    count: number
    total_available: number
    next_cursor: string | null
    source: string
    cached: boolean
    computed_at_ms: number
}

// ── Signals ───────────────────────────────────────────────────────────────────

export interface SignalItem {
    id: string
    timestamp_ms: number
    signal_type: SignalType
    confidence: number
    entry_price: number
    target_price: number
    stop_loss: number
    reason: string
    source: SignalSource
    reliability: ReliabilityLevel
    fallback_reason: string | null
}

export interface SignalsResponse {
    symbol: string
    from_ms: number
    to_ms: number
    signals: SignalItem[]
    count: number
}

// ── Backtest ──────────────────────────────────────────────────────────────────

export interface BacktestRequest {
    request_id: string
    symbol: string
    strategy_name: string
    from_ms: number
    to_ms: number
    initial_capital: number
    position_size_percent: number
}

export interface BacktestMetrics {
    total_trades: number
    winning_trades: number
    losing_trades: number
    win_rate: number
    profit_factor: number
    max_drawdown: number
    sharpe_ratio: number
    annual_return: number
}

export interface BacktestResponse {
    backtest_id: string
    symbol: string
    strategy_name: string
    from_ms: number
    to_ms: number
    initial_capital: number
    final_capital: number
    metrics: BacktestMetrics
    created_at_ms: number
}

// ── Health ────────────────────────────────────────────────────────────────────

export interface HealthResponse {
    status: HealthStatus
    timestamp_ms: number
    components: Record<string, string>
    version: string
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

export interface ApiError {
    error_code: ErrorCode
    message: string
    fallback_available: boolean
    timestamp_ms: number
    request_id: string | null
}