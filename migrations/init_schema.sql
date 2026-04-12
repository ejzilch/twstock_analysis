-- 建立動態股票清單資料表
CREATE TABLE IF NOT EXISTS symbols (
    symbol VARCHAR(20) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    exchange VARCHAR(10) NOT NULL,
    data_source VARCHAR(20) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    updated_at_ms BIGINT NOT NULL
);

-- 建立 K 線歷史數據資料表
CREATE TABLE IF NOT EXISTS candles (
    symbol VARCHAR(20) NOT NULL,
    timestamp_ms BIGINT NOT NULL,
    open DOUBLE PRECISION NOT NULL,
    high DOUBLE PRECISION NOT NULL,
    low DOUBLE PRECISION NOT NULL,
    close DOUBLE PRECISION NOT NULL,
    volume BIGINT NOT NULL,
    source VARCHAR(20) NOT NULL,

    -- 複合主鍵，支援 ON CONFLICT DO NOTHING 冪等性寫入
    PRIMARY KEY (symbol, timestamp_ms)
);

-- 建立時間戳索引，加速 from_ms 與 to_ms 的範圍查詢
CREATE INDEX IF NOT EXISTS idx_candles_timestamp
ON candles (timestamp_ms);