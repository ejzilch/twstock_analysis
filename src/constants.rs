/// 全域常數定義。所有業務數值、錯誤碼字串、設定值一律在此定義。
/// 禁止在程式碼任何地方硬寫數字或錯誤碼字串。

// ── API 錯誤碼 ────────────────────────────────────────────────────────────────
pub const ERROR_AI_SERVICE_TIMEOUT: &str = "AI_SERVICE_TIMEOUT";
pub const ERROR_AI_SERVICE_UNAVAILABLE: &str = "AI_SERVICE_UNAVAILABLE";
/// 手動同步相關錯誤碼
pub const ERROR_SYNC_ALREADY_RUNNING: &str = "SYNC_ALREADY_RUNNING";
pub const ERROR_SYNC_NOT_FOUND: &str = "SYNC_NOT_FOUND";

// ── candles 查詢限制 ──────────────────────────────────────────────────────────

/// 單次請求最多回傳 K 線數量，超過回傳 400 QUERY_RANGE_TOO_LARGE
pub const CANDLES_MAX_PER_REQUEST: usize = 2000;

// ── BulkInsertBuffer ──────────────────────────────────────────────────────────

/// 攢批觸發條件：累積筆數上限
pub const BULK_INSERT_MAX_BATCH_SIZE: usize = 500;

/// 攢批觸發條件：距上次 flush 的最大等待時間（毫秒）
pub const BULK_INSERT_MAX_WAIT_MS: u64 = 1_000;

// ── FinMind API ───────────────────────────────────────────────────────────────

/// FinMind API 回傳的日期字串格式
pub const FINMIND_DATE_FORMAT: &str = "%Y-%m-%d";

/// FinMind API URL get from .env
pub const FINMIND_API_BASE_URL: &str = "FINMIND_API_BASE_URL";

/// FinMind API token 環境變數名稱
pub const FINMIND_API_TOKEN_ENV: &str = "FINMIND_API_TOKEN";

/// FinMind API 請求 timeout（秒）
pub const FINMIND_API_TIMEOUT_SECS: u64 = 30;

/// FinMind 免費方案每小時請求上限
pub const FINMIND_RATE_LIMIT_PER_HOUR: u32 = 600;

/// Rate limit 安全緩衝：保留 10 次，避免邊界誤差
/// 實際觸發等待的閾值為：FINMIND_RATE_LIMIT_PER_HOUR - FINMIND_RATE_LIMIT_BUFFER = 590
pub const FINMIND_RATE_LIMIT_BUFFER: u32 = 10;

/// Rate limit 觸發後等待時間（秒）：1 小時
pub const FINMIND_RATE_LIMIT_WAIT_SECS: u64 = 3_600;

/// 手動補資料每批請求天數（約一個月）
pub const MANUAL_SYNC_BATCH_DAYS: u32 = 30;

// ── Redis ─────────────────────────────────────────────────────────────────────

/// Redis sync 狀態 key 前綴，完整格式：admin_sync:{sync_id}
pub const REDIS_SYNC_KEY_PREFIX: &str = "admin_sync";

/// Redis sync 狀態 TTL（秒）：24 小時
pub const REDIS_SYNC_TTL_SECS: u64 = 86_400;

// ── AI Service ────────────────────────────────────────────────────────────────

/// Python AI Service 請求 timeout（秒）
pub const AI_SERVICE_TIMEOUT_SECS: u64 = 10;

// ── Observability 告警閾值 ────────────────────────────────────────────────────

/// 最新 K 線距當前時間差超過此值（秒）觸發 warning
pub const OBSERVABILITY_DATA_LATENCY_WARNING_SECS: u64 = 300;

/// AI 推論 P99 耗時超過此值（毫秒）觸發 warning
pub const OBSERVABILITY_AI_INFERENCE_WARNING_MS: u64 = 200;

/// 最近 1 小時 API 成功率低於此值（%）觸發 warning
pub const OBSERVABILITY_SUCCESS_RATE_WARNING_PCT: f64 = 99.0;

/// 最近 1 小時 BridgeError 發生次數超過此值觸發 warning
pub const OBSERVABILITY_BRIDGE_ERRORS_WARNING_COUNT: u32 = 5;

// ── RSI 計算 ──────────────────────────────────────────────────────────────────

/// RSI 最大值，avg_loss 為 0（全部上漲）時回傳此值
pub const RSI_MAX_VALUE: f64 = 100.0;

// ── Rate Limiting 設定 ────────────────────────────────────────────────────────

/// 每個 IP 每分鐘最大請求數
pub const RATE_LIMIT_MAX_REQUESTS_PER_MINUTE: u32 = 60;

/// Rate Limit 滑動視窗大小（秒）
pub const RATE_LIMIT_WINDOW_SECS: u64 = 60;

// ── 版本 ──────────────────────────────────────────────────────────────────────

/// API 版本號，對應 openapi.yaml 的 version 欄位
pub const API_VERSION: &str = "2.2.0";

// ── 測試 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_buffer_smaller_than_per_hour_limit() {
        assert!(FINMIND_RATE_LIMIT_BUFFER < FINMIND_RATE_LIMIT_PER_HOUR);
    }

    #[test]
    fn test_bulk_insert_max_batch_size_is_500() {
        assert_eq!(BULK_INSERT_MAX_BATCH_SIZE, 500);
    }

    #[test]
    fn test_candles_max_per_request_is_2000() {
        assert_eq!(CANDLES_MAX_PER_REQUEST, 2000);
    }

    #[test]
    fn test_redis_sync_ttl_is_24_hours() {
        assert_eq!(REDIS_SYNC_TTL_SECS, 86_400);
    }

    #[test]
    fn test_ai_service_timeout_is_10_secs() {
        assert_eq!(AI_SERVICE_TIMEOUT_SECS, 10);
    }
}
