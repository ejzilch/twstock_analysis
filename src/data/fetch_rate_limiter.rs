/// src/data/fetch_rate_limiter.rs
///
/// FinMind API 排程限流器。
///
/// 在現有基礎上擴充：
///   - 新增 async 等待機制（達上限後等待 1 小時自動繼續）
///   - 新增進度記錄（記錄當前處理到哪一檔、哪個粒度、哪一天）
///   - 排程與手動同步共用同一個實例（由 AppState 持有）
///
/// ApiTier 付費升級介面維持不動（Gemini 原始設計）。
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::constants::{
    FINMIND_RATE_LIMIT_BUFFER, FINMIND_RATE_LIMIT_PER_HOUR, FINMIND_RATE_LIMIT_WAIT_SECS,
};

// ── ApiTier（Gemini 原始設計，維持不動）──────────────────────────────────────

/// FinMind API 付費等級，預留付費升級切換點。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiTier {
    Free,
    Paid,
}

/// Rate limit 設定，依 ApiTier 不同而異。
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 每分鐘最大請求數
    pub max_requests_per_minute: u32,
    /// 每日最大請求數
    pub max_requests_per_hour: u32,
    /// 目前使用的付費等級
    pub upgrade_tier: ApiTier,
}

impl RateLimitConfig {
    pub fn for_tier(tier: ApiTier) -> Self {
        match tier {
            ApiTier::Free => Self {
                max_requests_per_minute: 10,
                max_requests_per_hour: 600,
                upgrade_tier: ApiTier::Free,
            },
            ApiTier::Paid => Self {
                max_requests_per_minute: 60,
                max_requests_per_hour: 1_500,
                upgrade_tier: ApiTier::Paid,
            },
        }
    }
}

// ── 進度記錄（手動同步用）────────────────────────────────────────────────────

/// 目前處理的進度點，rate limit 等待後從此繼續。
#[derive(Debug, Clone, Default)]
pub struct SyncProgress {
    /// 目前處理的股票代號
    pub current_symbol: String,
    /// 目前處理的 K 線粒度
    pub current_interval: String,
    /// 目前補到哪一天（格式 "YYYY-MM-DD"）
    pub current_date: String,
}

// ── FinMindRateLimiter ────────────────────────────────────────────────────────

/// FinMind API 限流器。
///
/// 排程與手動同步共用同一個實例，確保合計請求數不超過 FinMind 上限。
/// 使用 Arc<Mutex<>> 確保多個 tokio task 安全共用。
#[derive(Debug)]
pub struct FinMindRateLimiter {
    config: RateLimitConfig,
    request_counter: AtomicU32,
    window_start: Mutex<Instant>,
    /// 達到 rate limit 時的等待結束時間（None 表示目前不在等待）
    resume_at: Mutex<Option<Instant>>,
    /// 最後記錄的進度（供等待後繼續使用）
    last_progress: Mutex<SyncProgress>,
}

impl FinMindRateLimiter {
    pub fn new(tier: ApiTier) -> Arc<Self> {
        Arc::new(Self {
            config: RateLimitConfig::for_tier(tier),
            request_counter: AtomicU32::new(0),
            window_start: Mutex::new(Instant::now()),
            resume_at: Mutex::new(None),
            last_progress: Mutex::new(SyncProgress::default()),
        })
    }

    /// 升級 ApiTier（付費後呼叫）。
    /// 預留介面，實際切換邏輯由 Gemini 決定。
    pub fn upgrade_tier(&mut self, new_tier: ApiTier) {
        self.config = RateLimitConfig::for_tier(new_tier);
        info!(tier = ?new_tier, "FinMind ApiTier upgraded");
    }

    /// 取得目前每小時已使用的請求次數。
    pub fn used_this_hour(&self) -> u32 {
        self.request_counter.load(Ordering::Relaxed)
    }

    /// 取得 rate limit 上限。
    pub fn limit_per_hour(&self) -> u32 {
        FINMIND_RATE_LIMIT_PER_HOUR
    }

    /// 是否目前在等待 rate limit 解除。
    pub async fn is_waiting(&self) -> bool {
        let resume_at = self.resume_at.lock().await;
        resume_at.is_some()
    }

    /// 若在等待中，回傳等待結束的毫秒級 timestamp。
    pub async fn resume_at_ms(&self) -> Option<i64> {
        let resume_at = self.resume_at.lock().await;
        resume_at.map(|t| {
            let remaining = t.saturating_duration_since(Instant::now());
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            now_ms + remaining.as_millis() as i64
        })
    }

    /// 記錄目前的同步進度（rate limit 等待後從此繼續）。
    pub async fn record_progress(&self, progress: SyncProgress) {
        let mut last = self.last_progress.lock().await;
        *last = progress;
    }

    /// 取得最後記錄的進度。
    pub async fn last_progress(&self) -> SyncProgress {
        self.last_progress.lock().await.clone()
    }

    /// 在執行每次 FinMind 請求前呼叫。
    ///
    /// 行為：
    ///   1. 檢查視窗是否已過 1 小時，若是則重置計數器
    ///   2. 若計數器達到安全上限（上限 - BUFFER），進入等待
    ///   3. 等待結束後重置計數器，允許繼續
    ///   4. 回傳 Ok（實際計數由 mark_request_used() 在成功請求後累加）
    pub async fn acquire(&self) -> Result<(), RateLimitWaiting> {
        // 重置視窗
        {
            let mut window_start = self.window_start.lock().await;
            if window_start.elapsed() >= Duration::from_secs(FINMIND_RATE_LIMIT_WAIT_SECS) {
                self.request_counter.store(0, Ordering::Relaxed);
                *window_start = Instant::now();
                let mut resume_at = self.resume_at.lock().await;
                *resume_at = None;
                info!("FinMind rate limit window reset");
            }
        }

        let current = self.request_counter.load(Ordering::Relaxed);
        let safe_limit = FINMIND_RATE_LIMIT_PER_HOUR - FINMIND_RATE_LIMIT_BUFFER;

        if current >= safe_limit {
            // 計算需要等待的時間
            let wait_duration = Duration::from_secs(FINMIND_RATE_LIMIT_WAIT_SECS);
            let resume_instant = Instant::now() + wait_duration;

            {
                let mut resume_at = self.resume_at.lock().await;
                *resume_at = Some(resume_instant);
            }

            warn!(
                used = current,
                limit = FINMIND_RATE_LIMIT_PER_HOUR,
                wait_secs = FINMIND_RATE_LIMIT_WAIT_SECS,
                "FinMind rate limit reached, waiting"
            );

            // 非同步等待，不阻塞其他 task
            tokio::time::sleep(wait_duration).await;

            // 等待結束，重置
            self.request_counter.store(0, Ordering::Relaxed);
            {
                let mut window_start = self.window_start.lock().await;
                *window_start = Instant::now();
                let mut resume_at = self.resume_at.lock().await;
                *resume_at = None;
            }

            info!("FinMind rate limit wait complete, resuming");
        }

        self.request_counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 在確認請求有效送出後呼叫，累加本地使用量統計。
    pub fn mark_request_used(&self) {
        self.request_counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// acquire() 回傳的等待狀態（目前設計為直接 async 等待，此型別保留供未來擴充）。
#[derive(Debug)]
pub struct RateLimitWaiting {
    pub resume_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_increments_counter() {
        let limiter = FinMindRateLimiter::new(ApiTier::Free);
        assert_eq!(limiter.used_this_hour(), 0);
        limiter.acquire().await.unwrap();
        assert_eq!(limiter.used_this_hour(), 1);
    }

    #[tokio::test]
    async fn test_is_not_waiting_initially() {
        let limiter = FinMindRateLimiter::new(ApiTier::Free);
        assert!(!limiter.is_waiting().await);
    }

    #[test]
    fn test_rate_limit_config_free_tier() {
        let config = RateLimitConfig::for_tier(ApiTier::Free);
        assert_eq!(config.max_requests_per_hour, 600);
    }

    #[test]
    fn test_rate_limit_config_paid_tier() {
        let config = RateLimitConfig::for_tier(ApiTier::Paid);
        assert_eq!(config.max_requests_per_hour, 1_500);
    }
}
