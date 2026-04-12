use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Copy)]
pub enum ApiTier {
    Free,
    Paid,
}

pub struct FinMindRateLimiter {
    semaphore: Arc<Semaphore>,
    tier: ApiTier,
    last_request_at: Arc<Mutex<Instant>>, // 使用 Mutex 保護最後請求時間，確保間隔冷卻
}

impl FinMindRateLimiter {
    pub fn new(tier: ApiTier) -> Self {
        let max_concurrent = match tier {
            ApiTier::Free => 1, // 免費版使用低併發
            ApiTier::Paid => 10,
        };

        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            tier,
            last_request_at: Arc::new(Mutex::new(Instant::now() - Duration::from_secs(60))),
        }
    }

    /// 獲取許可證，同時處理併發與間隔冷卻
    pub async fn acquire(&self) -> anyhow::Result<tokio::sync::SemaphorePermit<'_>> {
        // 1. 先取得併發許可
        let permit = self.semaphore.acquire().await?;

        // 2. 處理間隔冷卻 (Cooldown)
        let mut last_req = self.last_request_at.lock().await;
        let now = Instant::now();
        let wait_duration = self.get_wait_duration();

        let elapsed = now.duration_since(*last_req);
        if elapsed < wait_duration {
            tokio::time::sleep(wait_duration - elapsed).await;
        }

        *last_req = Instant::now();
        Ok(permit)
    }

    /// 依據 ApiTier 取得強制等待間隔
    pub fn get_wait_duration(&self) -> Duration {
        match self.tier {
            ApiTier::Free => Duration::from_secs(2), // 免費版每 2 秒一次
            ApiTier::Paid => Duration::from_millis(100), // 付費版每秒 10 次
        }
    }
}
