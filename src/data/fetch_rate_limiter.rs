use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// ── 公開型別 ────────────────────────────────────────────────────────────────

/// FinMind API 方案等級
///
/// 升級付費方案時只需將 ApiTier 從 Free 改為 Paid，
/// RateLimitConfig 會自動套用對應限額，不需修改呼叫邏輯。
#[derive(Debug, Clone, PartialEq)]
pub enum ApiTier {
    Free,
    Paid,
}

/// 限流設定
///
/// 免費方案預設值由 RateLimitConfig::free() 建立。
/// 付費方案由 RateLimitConfig::paid() 建立。
/// 兩者介面相同，切換時不影響 FinMindRateLimiter 的使用方式。
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub tier: ApiTier,
    pub max_requests_per_minute: u32,
    pub max_requests_per_day: u32,
}

impl RateLimitConfig {
    /// 免費方案設定（預設使用）
    pub fn free() -> Self {
        Self {
            tier: ApiTier::Free,
            max_requests_per_minute: 10,
            max_requests_per_day: 1_000,
        }
    }

    /// 付費方案設定（升級後切換）
    pub fn paid() -> Self {
        Self {
            tier: ApiTier::Paid,
            max_requests_per_minute: 100,
            max_requests_per_day: 100_000,
        }
    }
}

/// FinMind API 排程限流器
///
/// 追蹤每分鐘與每日請求量，達到上限時回傳 RateLimitError，
/// 由 fetch.rs 的呼叫方決定是否切換至 yfinance 備用來源。
///
/// 使用方式：
/// ```rust
/// let limiter = FinMindRateLimiter::new(RateLimitConfig::free());
/// match limiter.acquire().await {
///     Ok(()) => { /* 發出 FinMind 請求 */ }
///     Err(RateLimitError::MinuteQuotaExceeded) => { /* 切換 yfinance */ }
///     Err(RateLimitError::DailyQuotaExceeded)  => { /* 切換 yfinance，記錄告警 */ }
/// }
/// ```
#[derive(Debug)]
pub struct FinMindRateLimiter {
    config: Arc<RateLimitConfig>,
    minute_request_count: Arc<AtomicU32>,
    daily_request_count: Arc<AtomicU32>,
    minute_window_state: Arc<Mutex<MinuteWindowState>>,
    day_window_state: Arc<Mutex<DayWindowState>>,
}

// ── 內部狀態 ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct MinuteWindowState {
    window_start: Instant,
}

#[derive(Debug)]
struct DayWindowState {
    window_start: Instant,
}

// ── 錯誤型別 ─────────────────────────────────────────────────────────────────

/// 限流錯誤，由 fetch.rs 接收後決定降級策略
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("FinMind minute quota exceeded: {current}/{limit} requests this minute")]
    MinuteQuotaExceeded { current: u32, limit: u32 },

    #[error("FinMind daily quota exceeded: {current}/{limit} requests today")]
    DailyQuotaExceeded { current: u32, limit: u32 },
}

// ── 實作 ──────────────────────────────────────────────────────────────────────

impl FinMindRateLimiter {
    /// 建立新的限流器，傳入設定決定方案等級
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config: Arc::new(config),
            minute_request_count: Arc::new(AtomicU32::new(0)),
            daily_request_count: Arc::new(AtomicU32::new(0)),
            minute_window_state: Arc::new(Mutex::new(MinuteWindowState {
                window_start: Instant::now(),
            })),
            day_window_state: Arc::new(Mutex::new(DayWindowState {
                window_start: Instant::now(),
            })),
        }
    }

    /// 嘗試取得一個請求配額
    ///
    /// 成功回傳 Ok(())，呼叫方可繼續發出 FinMind 請求。
    /// 失敗回傳 RateLimitError，呼叫方應切換至 yfinance 備用來源。
    pub async fn acquire(&self) -> Result<(), RateLimitError> {
        self.reset_minute_window_if_elapsed().await;
        self.reset_day_window_if_elapsed().await;
        self.check_minute_quota()?;
        self.check_daily_quota()?;

        self.minute_request_count.fetch_add(1, Ordering::Relaxed);
        self.daily_request_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// 每日剩餘配額百分比，供 /health/integrity 的 rate_limit_remaining_pct 欄位使用
    pub fn daily_remaining_pct(&self) -> f64 {
        let used = self.daily_request_count.load(Ordering::Relaxed) as f64;
        let limit = self.config.max_requests_per_day as f64;
        let remaining = (limit - used).max(0.0);
        (remaining / limit * 100.0).clamp(0.0, 100.0)
    }

    /// 目前使用的方案等級，供日誌與監控使用
    pub fn tier(&self) -> &ApiTier {
        &self.config.tier
    }

    // ── 私有方法 ────────────────────────────────────────────────────────────

    /// 若距上次分鐘視窗開始已超過 60 秒，重置分鐘計數器
    async fn reset_minute_window_if_elapsed(&self) {
        let mut state = self.minute_window_state.lock().await;
        if state.window_start.elapsed() >= Duration::from_secs(60) {
            self.minute_request_count.store(0, Ordering::Relaxed);
            state.window_start = Instant::now();
        }
    }

    /// 若距上次日視窗開始已超過 24 小時，重置每日計數器
    async fn reset_day_window_if_elapsed(&self) {
        let mut state = self.day_window_state.lock().await;
        if state.window_start.elapsed() >= Duration::from_secs(86_400) {
            self.daily_request_count.store(0, Ordering::Relaxed);
            state.window_start = Instant::now();
        }
    }

    fn check_minute_quota(&self) -> Result<(), RateLimitError> {
        let current = self.minute_request_count.load(Ordering::Relaxed);
        let limit = self.config.max_requests_per_minute;
        if current >= limit {
            return Err(RateLimitError::MinuteQuotaExceeded { current, limit });
        }
        Ok(())
    }

    fn check_daily_quota(&self) -> Result<(), RateLimitError> {
        let current = self.daily_request_count.load(Ordering::Relaxed);
        let limit = self.config.max_requests_per_day;
        if current >= limit {
            return Err(RateLimitError::DailyQuotaExceeded { current, limit });
        }
        Ok(())
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_within_quota_succeeds() {
        let limiter = FinMindRateLimiter::new(RateLimitConfig::free());
        assert!(limiter.acquire().await.is_ok());
    }

    #[tokio::test]
    async fn test_minute_quota_exceeded_returns_error() {
        let config = RateLimitConfig {
            tier: ApiTier::Free,
            max_requests_per_minute: 2,
            max_requests_per_day: 1_000,
        };
        let limiter = FinMindRateLimiter::new(config);

        assert!(limiter.acquire().await.is_ok());
        assert!(limiter.acquire().await.is_ok());

        let result = limiter.acquire().await;
        assert!(matches!(
            result,
            Err(RateLimitError::MinuteQuotaExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn test_daily_quota_exceeded_returns_error() {
        let config = RateLimitConfig {
            tier: ApiTier::Free,
            max_requests_per_minute: 100,
            max_requests_per_day: 2,
        };
        let limiter = FinMindRateLimiter::new(config);

        assert!(limiter.acquire().await.is_ok());
        assert!(limiter.acquire().await.is_ok());

        let result = limiter.acquire().await;
        assert!(matches!(
            result,
            Err(RateLimitError::DailyQuotaExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn test_daily_remaining_pct_decreases_with_usage() {
        let config = RateLimitConfig {
            tier: ApiTier::Free,
            max_requests_per_minute: 100,
            max_requests_per_day: 10,
        };
        let limiter = FinMindRateLimiter::new(config);

        assert!((limiter.daily_remaining_pct() - 100.0).abs() < f64::EPSILON);

        limiter.acquire().await.unwrap();
        assert!((limiter.daily_remaining_pct() - 90.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_paid_tier_has_higher_quota() {
        let free_config = RateLimitConfig::free();
        let paid_config = RateLimitConfig::paid();
        assert!(paid_config.max_requests_per_day > free_config.max_requests_per_day);
        assert!(paid_config.max_requests_per_minute > free_config.max_requests_per_minute);
    }
}
