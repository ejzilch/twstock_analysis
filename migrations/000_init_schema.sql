SET client_encoding = 'UTF8';
-- 動態股票清單
CREATE TABLE IF NOT EXISTS symbols (
    symbol          VARCHAR(20)  PRIMARY KEY,
    name            VARCHAR(100) NOT NULL,
    exchange        VARCHAR(10)  NOT NULL,
    data_source     VARCHAR(20)  NOT NULL,
    earliest_ms     BIGINT       NOT NULL DEFAULT 0,
    latest_ms       BIGINT       NOT NULL DEFAULT 0,
    is_active       BOOLEAN      NOT NULL DEFAULT true,
    updated_at_ms   BIGINT       NOT NULL
);

-- K 線歷史數據
-- 複合主鍵含 interval，避免不同時間粒度資料互相覆蓋
CREATE TABLE IF NOT EXISTS candles (
    symbol          VARCHAR(20)  NOT NULL,
    timestamp_ms    BIGINT       NOT NULL,
    interval        VARCHAR(5)   NOT NULL, -- 1m / 5m / 15m / 1h / 4h / 1d
    open            DOUBLE PRECISION NOT NULL,
    high            DOUBLE PRECISION NOT NULL,
    low             DOUBLE PRECISION NOT NULL,
    close           DOUBLE PRECISION NOT NULL,
    volume          BIGINT       NOT NULL,
    source          VARCHAR(20)  NOT NULL,

    PRIMARY KEY (symbol, timestamp_ms, interval)
);

-- 範圍查詢的核心索引：WHERE symbol = ? AND interval = ? AND timestamp_ms BETWEEN ? AND ?
CREATE INDEX IF NOT EXISTS idx_candles_symbol_interval_timestamp
ON candles (symbol, interval, timestamp_ms);

-- 交易信號
CREATE TABLE IF NOT EXISTS signals (
    id              VARCHAR(50)  PRIMARY KEY,
    symbol          VARCHAR(20)  NOT NULL,
    timestamp_ms    BIGINT       NOT NULL,
    signal_type     VARCHAR(10)  NOT NULL, -- BUY / SELL
    confidence      DOUBLE PRECISION NOT NULL,
    entry_price     DOUBLE PRECISION NOT NULL,
    target_price    DOUBLE PRECISION NOT NULL,
    stop_loss       DOUBLE PRECISION NOT NULL,
    reason          TEXT         NOT NULL,
    source          VARCHAR(30)  NOT NULL, -- ai_ensemble / technical_only / manual_override
    reliability     VARCHAR(10)  NOT NULL, -- high / medium / low / unknown
    fallback_reason VARCHAR(50),
    created_at_ms   BIGINT       NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_signals_symbol_timestamp
ON signals (symbol, timestamp_ms);

-- 回測結果
CREATE TABLE IF NOT EXISTS backtest_results (
    backtest_id         VARCHAR(50)  PRIMARY KEY,
    symbol              VARCHAR(20)  NOT NULL,
    strategy_name       VARCHAR(100) NOT NULL,
    from_ms             BIGINT       NOT NULL,
    to_ms               BIGINT       NOT NULL,
    initial_capital     DOUBLE PRECISION NOT NULL,
    final_capital       DOUBLE PRECISION NOT NULL,
    total_trades        INT          NOT NULL,
    winning_trades      INT          NOT NULL,
    losing_trades       INT          NOT NULL,
    win_rate            DOUBLE PRECISION NOT NULL,
    profit_factor       DOUBLE PRECISION NOT NULL,
    max_drawdown        DOUBLE PRECISION NOT NULL,
    sharpe_ratio        DOUBLE PRECISION NOT NULL,
    annual_return       DOUBLE PRECISION NOT NULL,
    created_at_ms       BIGINT       NOT NULL
);