/// 系統全域常數
///
/// 集中管理所有跨模組使用的字串常數與數值常數，
/// 禁止在程式碼中硬寫這些值，一律引用此模組。

// ── API 錯誤碼 ────────────────────────────────────────────────────────────────
// 對應 API_CONTRACT.md 的錯誤碼清單與 src/api/middleware/error.rs 的 ApiError

/// 認證失敗，X-API-KEY 缺少或無效
pub const ERROR_UNAUTHORIZED: &str = "UNAUTHORIZED";

/// AI 服務請求超時，已自動降級至技術指標
pub const ERROR_AI_SERVICE_TIMEOUT: &str = "AI_SERVICE_TIMEOUT";

/// AI 服務不可用
pub const ERROR_AI_SERVICE_UNAVAILABLE: &str = "AI_SERVICE_UNAVAILABLE";

/// 外部資料源中斷
pub const ERROR_DATA_SOURCE_INTERRUPTED: &str = "DATA_SOURCE_INTERRUPTED";

/// 外部資料源達到請求上限，系統切換備援中
pub const ERROR_DATA_SOURCE_RATE_LIMITED: &str = "DATA_SOURCE_RATE_LIMITED";

/// 指標計算失敗
pub const ERROR_INDICATOR_COMPUTE_FAILED: &str = "INDICATOR_COMPUTE_FAILED";

/// Redis 快取未命中，已直接計算，靜默處理
pub const ERROR_CACHE_MISS_FALLBACK: &str = "CACHE_MISS_FALLBACK";

/// 數值計算溢位，超出安全範圍
pub const ERROR_COMPUTATION_OVERFLOW: &str = "COMPUTATION_OVERFLOW";

/// 指標設定格式錯誤
pub const ERROR_INVALID_INDICATOR_CONFIG: &str = "INVALID_INDICATOR_CONFIG";

/// 找不到指定股票代號
pub const ERROR_SYMBOL_NOT_FOUND: &str = "SYMBOL_NOT_FOUND";

/// 查詢範圍超過單次上限（2000 根 K 線）
pub const ERROR_QUERY_RANGE_TOO_LARGE: &str = "QUERY_RANGE_TOO_LARGE";

// ── K 線查詢限制 ──────────────────────────────────────────────────────────────
// 對應 API_CONTRACT.md 的數量限制規範

/// 單次 candles 查詢最多回傳的 K 線數量
pub const CANDLES_MAX_QUERY_LIMIT: usize = 2000;

// ── Timeout 設定（秒） ────────────────────────────────────────────────────────
// 對應 ARCH_DESIGN.md 的超時與降級策略

/// Python AI Service 請求 timeout
pub const TIMEOUT_AI_SERVICE_SECS: u64 = 10;

/// PostgreSQL 查詢 timeout
pub const TIMEOUT_POSTGRES_SECS: u64 = 5;

/// Redis 操作 timeout
pub const TIMEOUT_REDIS_SECS: u64 = 1;

/// FinMind API 請求 timeout
pub const TIMEOUT_FINMIND_SECS: u64 = 15;

// ── Bulk Insert 設定 ──────────────────────────────────────────────────────────
// 對應 ARCH_DESIGN.md 的 BulkInsertBuffer 設計

/// 攢批寫入的最大筆數，達到此數量立即刷入
pub const BULK_INSERT_MAX_BATCH_SIZE: usize = 500;

/// 攢批寫入的最大等待時間（毫秒），超過此時間立即刷入
pub const BULK_INSERT_MAX_WAIT_MS: u64 = 1000;

// ── Rate Limiting 設定 ────────────────────────────────────────────────────────

/// 每個 IP 每分鐘最大請求數
pub const RATE_LIMIT_MAX_REQUESTS_PER_MINUTE: u32 = 60;

/// Rate Limit 滑動視窗大小（秒）
pub const RATE_LIMIT_WINDOW_SECS: u64 = 60;

// ── FinMind Rate Limit 設定 ───────────────────────────────────────────────────
// 對應 fetch_rate_limiter.rs 的 RateLimitConfig

/// FinMind 免費方案每分鐘請求上限
pub const FINMIND_FREE_MAX_REQUESTS_PER_MINUTE: u32 = 10;

/// FinMind 免費方案每日請求上限
pub const FINMIND_FREE_MAX_REQUESTS_PER_DAY: u32 = 1_000;

/// FinMind 付費方案每分鐘請求上限
pub const FINMIND_PAID_MAX_REQUESTS_PER_MINUTE: u32 = 100;

/// FinMind 付費方案每日請求上限
pub const FINMIND_PAID_MAX_REQUESTS_PER_DAY: u32 = 100_000;

// ── FinMind API 設定 ─────────────────────────────────────────────────────────

/// FinMind API 回傳的日期字串格式
pub const FINMIND_DATE_FORMAT: &str = "%Y-%m-%d";

/// FinMind stock_type 欄位中代表台灣證券交易所的值
pub const FINMIND_EXCHANGE_TWSE: &str = "twse";

/// FinMind stock_type 欄位中代表證券櫃檯買賣中心的值
pub const FINMIND_EXCHANGE_TPEX: &str = "tpex";

// ── Observability 告警閾值 ────────────────────────────────────────────────────
// 對應 API_CONTRACT.md 的 Observability 欄位說明

/// data_latency_seconds 的 warning 閾值（秒）
pub const OBSERVABILITY_DATA_LATENCY_WARNING_SECS: i64 = 300;

/// ai_inference_p99_ms 的 warning 閾值（毫秒）
pub const OBSERVABILITY_AI_INFERENCE_WARNING_MS: i64 = 200;

/// api_success_rate_pct 的 warning 閾值（百分比）
pub const OBSERVABILITY_SUCCESS_RATE_WARNING_PCT: f64 = 99.0;

/// bridge_errors_last_hour 的 warning 閾值（次數）
pub const OBSERVABILITY_BRIDGE_ERRORS_WARNING_COUNT: i64 = 5;

// ── Redis 快取 TTL（秒） ──────────────────────────────────────────────────────
// 對應 ARCH_DESIGN.md 的快取鍵設計

/// stock:{symbol}:latest 的 TTL
pub const CACHE_TTL_STOCK_LATEST_SECS: u64 = 300; // 5 分鐘

/// indicators:{symbol}:{interval} 的 TTL
pub const CACHE_TTL_INDICATORS_SECS: u64 = 600; // 10 分鐘

/// signal:{symbol}:* 的 TTL
pub const CACHE_TTL_SIGNAL_SECS: u64 = 86_400; // 24 小時

/// symbols:active 的 TTL
pub const CACHE_TTL_SYMBOLS_ACTIVE_SECS: u64 = 600; // 10 分鐘

// ── Redis 操作 ────────────────────────────────────────────────────────────────

/// SCAN 指令每批最多取回的 key 數量
/// 避免單次 SCAN 佔用 Redis 過長時間，參考 Redis 官方建議值
pub const REDIS_SCAN_BATCH_SIZE: usize = 100;

// ── Windows Duration 時間（秒） ──────────────────────────────────────────────────────

pub mod limits {
    use std::time::Duration;

    /// 視窗持續時間 1 分鐘
    pub const MINUTE_WINDOW_DURATION: Duration = Duration::from_secs(60);
    /// 視窗持續時間 1 天
    pub const DAY_WINDOW_DURATION: Duration = Duration::from_secs(86_400);
}

// ── RSI 計算 ──────────────────────────────────────────────────────────────────

/// RSI 最大值，avg_loss 為 0（全部上漲）時回傳此值
pub const RSI_MAX_VALUE: f64 = 100.0;

/// RSI 最小值，avg_gain 為 0（全部下跌）時回傳此值
pub const RSI_MIN_VALUE: f64 = 0.0;

// ── 版本資訊 ──────────────────────────────────────────────────────────────────

/// API 版本號，對應 openapi.yaml 的 version 欄位
pub const API_VERSION: &str = "2.2.0";

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candles_max_limit_is_2000() {
        assert_eq!(CANDLES_MAX_QUERY_LIMIT, 2000);
    }

    #[test]
    fn test_timeout_hierarchy_is_correct() {
        // Redis < PostgreSQL < AI Service < FinMind
        assert!(TIMEOUT_REDIS_SECS < TIMEOUT_POSTGRES_SECS);
        assert!(TIMEOUT_POSTGRES_SECS < TIMEOUT_AI_SERVICE_SECS);
        assert!(TIMEOUT_AI_SERVICE_SECS < TIMEOUT_FINMIND_SECS);
    }

    #[test]
    fn test_finmind_paid_quota_exceeds_free() {
        assert!(FINMIND_PAID_MAX_REQUESTS_PER_MINUTE > FINMIND_FREE_MAX_REQUESTS_PER_MINUTE);
        assert!(FINMIND_PAID_MAX_REQUESTS_PER_DAY > FINMIND_FREE_MAX_REQUESTS_PER_DAY);
    }

    #[test]
    fn test_bulk_insert_settings_are_positive() {
        assert!(BULK_INSERT_MAX_BATCH_SIZE > 0);
        assert!(BULK_INSERT_MAX_WAIT_MS > 0);
    }

    #[test]
    fn test_error_codes_are_non_empty() {
        let codes = [
            ERROR_UNAUTHORIZED,
            ERROR_AI_SERVICE_TIMEOUT,
            ERROR_AI_SERVICE_UNAVAILABLE,
            ERROR_DATA_SOURCE_INTERRUPTED,
            ERROR_DATA_SOURCE_RATE_LIMITED,
            ERROR_INDICATOR_COMPUTE_FAILED,
            ERROR_CACHE_MISS_FALLBACK,
            ERROR_COMPUTATION_OVERFLOW,
            ERROR_INVALID_INDICATOR_CONFIG,
            ERROR_SYMBOL_NOT_FOUND,
            ERROR_QUERY_RANGE_TOO_LARGE,
        ];
        for code in codes {
            assert!(!code.is_empty());
        }
    }
}
