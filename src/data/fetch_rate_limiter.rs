/// FinMind API 排程限流器。
///
/// 在現有基礎上擴充：
///   - 新增 async 等待機制（達上限後等待 1 小時自動繼續）
///   - 新增進度記錄（記錄當前處理到哪一檔、哪個粒度、哪一天）
///   - 排程與手動同步共用同一個實例（由 AppState 持有）
///
/// ApiTier 付費升級介面維持不動（Gemini 原始設計）。
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::constants::{
    FINMIND_RATE_LIMIT_BUFFER, FINMIND_RATE_LIMIT_PER_HOUR, FINMIND_RATE_LIMIT_RESUME_DELAY_SECS,
};

// ── FinMindRateLimiter ────────────────────────────────────────────────────────

/// FinMind API 限流器。
///
/// 排程與手動同步共用同一個實例，確保合計請求數不超過 FinMind 上限。
/// 使用 Arc<Mutex<>> 確保多個 tokio task 安全共用。
#[derive(Debug)]
pub struct FinMindRateLimiter {
    usage_timestamps: Mutex<VecDeque<Instant>>,
    /// 達到 rate limit 時的等待結束時間（None 表示目前不在等待）
    resume_at: Mutex<Option<Instant>>,
}

impl FinMindRateLimiter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            usage_timestamps: Mutex::new(VecDeque::new()),
            resume_at: Mutex::new(None),
        })
    }

    /// 取得目前每小時已使用的請求次數（清理過期紀錄後）。
    pub async fn used_this_hour(&self) -> u32 {
        let mut timestamps = self.usage_timestamps.lock().await;
        let now = Instant::now();
        let window = Duration::from_secs(3_600);

        while let Some(oldest) = timestamps.front() {
            if now.duration_since(*oldest) >= window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        timestamps.len() as u32
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

    /// 若目前呼叫 acquire() 會進入等待，回傳預估 resume_at_ms。
    pub async fn predicted_resume_at_ms(&self) -> Option<i64> {
        let safe_limit = FINMIND_RATE_LIMIT_PER_HOUR - FINMIND_RATE_LIMIT_BUFFER;
        let mut timestamps = self.usage_timestamps.lock().await;
        let now = Instant::now();
        let window = Duration::from_secs(3_600);

        while let Some(oldest) = timestamps.front() {
            if now.duration_since(*oldest) >= window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        if timestamps.len() < safe_limit as usize {
            return None;
        }

        let oldest = *timestamps.front()?;
        let release_time = oldest + window;
        let remaining = release_time.saturating_duration_since(now);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Some(now_ms + remaining.as_millis() as i64)
    }

    /// 在執行每次 FinMind 請求前呼叫。
    ///
    /// 行為：
    ///   1. 清理 1 小時前的使用紀錄（滑動視窗）
    ///   2. 若當前 1 小時內使用量達到安全上限（上限 - BUFFER），等待到最早那筆滿 1 小時
    ///   3. 等待結束後重試，直到可用
    pub async fn acquire(&self) {
        let safe_limit = FINMIND_RATE_LIMIT_PER_HOUR - FINMIND_RATE_LIMIT_BUFFER;
        loop {
            let maybe_wait_duration = {
                let mut timestamps = self.usage_timestamps.lock().await;
                let now = Instant::now();
                let window = Duration::from_secs(3_600);

                while let Some(oldest) = timestamps.front() {
                    if now.duration_since(*oldest) >= window {
                        timestamps.pop_front();
                    } else {
                        break;
                    }
                }

                if timestamps.len() < safe_limit as usize {
                    None
                } else {
                    timestamps.front().map(|oldest| {
                        let release_time = *oldest + window;
                        release_time.saturating_duration_since(now)
                    })
                }
            };

            if let Some(wait_duration) = maybe_wait_duration {
                let wait_with_delay =
                    wait_duration + Duration::from_secs(FINMIND_RATE_LIMIT_RESUME_DELAY_SECS);
                let resume_instant = Instant::now() + wait_with_delay;
                {
                    let mut resume_at = self.resume_at.lock().await;
                    *resume_at = Some(resume_instant);
                }

                tokio::time::sleep(wait_with_delay).await;
                {
                    let mut resume_at = self.resume_at.lock().await;
                    *resume_at = None;
                }
                continue;
            }

            break;
        }
    }

    /// 在確認請求有效送出後呼叫，累加本地使用量統計。
    pub async fn mark_request_used(&self) {
        let mut timestamps = self.usage_timestamps.lock().await;
        timestamps.push_back(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_not_waiting_initially() {
        let limiter = FinMindRateLimiter::new();
        assert!(!limiter.is_waiting().await);
    }
}
