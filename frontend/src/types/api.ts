// ==========================================
// AI Bridge API Interfaces
// Auto-generated from OpenAPI Spec v2.2.0
// ==========================================

// ------------------------------------------
// 1. System & Health
// ------------------------------------------
export interface HealthResponse {
    status: 'ok' | 'degraded';
    timestamp_ms: number;
    components: {
        database: string;
        redis: string;
        python_ai_service: string;
    };
    version: string;
}

export interface IntegrityResponse {
    status: string;
    timestamp_ms: number;
    checks: Record<string, any>;
    observability: {
        data_latency_seconds: number;
        data_latency_status: string;
        ai_inference_p99_ms: number;
        ai_inference_status: string;
        api_success_rate_pct: number;
        api_success_rate_status: string;
        bridge_errors_last_hour: number;
    };
}

// ------------------------------------------
// 2. Symbols & Market Data
// ------------------------------------------
export interface Symbol {
    symbol: string;
    name: string;
    exchange: string;
    data_source: string;
    earliest_available_ms: number;
    latest_available_ms: number;
    is_active: boolean;
}

export interface SymbolsResponse {
    symbols: Symbol[];
    count: number;
    last_synced_ms: number;
}

export interface Candle {
    timestamp_ms: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
    // 支援動態技術指標 (e.g., { "ma20": 150.5, "macd": { ... } })
    indicators: Record<string, any>;
}

export interface CandlesResponse {
    symbol: string;
    interval: string;
    from_ms: number;
    to_ms: number;
    candles: Candle[];
    count: number;
    total_available: number;
    next_cursor: string | null;
    source: string;
    cached: boolean;
    computed_at_ms: number;
}

// ------------------------------------------
// 3. Technical Indicators
// ------------------------------------------
export interface ComputeIndicatorsRequest {
    request_id: string;
    symbol: string;
    from_ms: number;
    to_ms: number;
    interval: string;
    indicators: Record<string, any>;
}

export interface ComputeIndicatorsResponse {
    symbol: string;
    interval: string;
    from_ms: number;
    to_ms: number;
    indicators: Record<string, any>;
    computed_at_ms: number;
    computation_time_ms: number;
    cached: boolean;
    dag_execution_order: string[];
}

// ------------------------------------------
// 4. Trading Signals & AI Predictions
// ------------------------------------------
export interface Signal {
    id: string;
    timestamp_ms: number;
    signal_type: 'BUY' | 'SELL' | 'HOLD';
    confidence: number;
    entry_price: number;
    target_price: number;
    stop_loss: number;
    reason: string;
    source: string;
    reliability: string;
    fallback_reason: string | null;
}

export interface SignalsResponse {
    symbol: string;
    from_ms: number;
    to_ms: number;
    signals: Signal[];
    count: number;
}

export interface PredictionRequest {
    request_id: string;
    symbol: string;
    indicators: Record<string, any>;
    lookback_hours: number;
}

export interface PredictionResponse {
    symbol: string;
    up_probability: number;
    down_probability: number;
    confidence_score: number;
    model_version: string;
    inference_time_ms: number;
    computed_at_ms: number;
}

// ------------------------------------------
// 5. Backtesting
// ------------------------------------------
export interface BacktestRequest {
    request_id: string;
    symbol: string;
    strategy_name: string;
    from_ms: number;
    to_ms: number;
    initial_capital: number;
    position_size_percent: number;
}

export interface BacktestResponse {
    backtest_id: string;
    symbol: string;
    strategy_name: string;
    from_ms: number;
    to_ms: number;
    initial_capital: number;
    final_capital: number;
    metrics: {
        total_trades: number;
        winning_trades: number;
        losing_trades: number;
        win_rate: number;
        profit_factor: number;
        max_drawdown: number;
        sharpe_ratio: number;
        annual_return: number;
    };
    created_at_ms: number;
}

// ------------------------------------------
// 6. Error Handling
// ------------------------------------------
export interface ErrorResponse {
    error_code: string;
    message: string;
    fallback_available: boolean;
    timestamp_ms: number;
    request_id: string | null;
}