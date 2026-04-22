/// 同步任務狀態的 Redis 存取層。
///
/// 改用 redis::aio::MultiplexedConnection（async），
/// 對應 AppState 中的 redis_client 型別。
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::api::models::admin_sync::{
    GapProgress, SymbolProgress, SymbolSyncStatus, SyncStatus, SyncSummary,
};
use crate::constants::{REDIS_SYNC_KEY_PREFIX, REDIS_SYNC_TTL_SECS};
use crate::core::BridgeError;

/// Redis 中儲存的完整同步狀態。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncState {
    pub sync_id: String,
    pub status: SyncStatus,
    pub started_at_ms: i64,
    pub symbols: Vec<String>,
    pub progress: Vec<SymbolProgress>,
    pub summary: SyncSummary,
}

impl SyncState {
    pub fn new(sync_id: String, symbols: Vec<String>, started_at_ms: i64) -> Self {
        let progress = symbols
            .iter()
            .map(|s| SymbolProgress {
                symbol: s.clone(),
                name: String::new(),
                status: SymbolSyncStatus::Pending,
                gap_a: None,
                gap_b: None,
            })
            .collect();

        let total = symbols.len();

        Self {
            sync_id,
            status: SyncStatus::Running,
            started_at_ms,
            symbols,
            progress,
            summary: SyncSummary {
                total_symbols: total,
                completed_symbols: 0,
                total_inserted: 0,
                total_skipped: 0,
                total_failed: 0,
            },
        }
    }

    pub fn is_in_progress(&self) -> bool {
        self.status.is_in_progress()
    }

    pub fn update_gap_a(&mut self, symbol: &str, gap: GapProgress) {
        if let Some(p) = self.progress.iter_mut().find(|p| p.symbol == symbol) {
            p.gap_a = Some(gap);
        }
    }

    pub fn update_gap_b(&mut self, symbol: &str, gap: GapProgress) {
        if let Some(p) = self.progress.iter_mut().find(|p| p.symbol == symbol) {
            p.gap_b = Some(gap);
        }
    }

    pub fn mark_symbol_completed(&mut self, symbol: &str) {
        if let Some(p) = self.progress.iter_mut().find(|p| p.symbol == symbol) {
            p.status = SymbolSyncStatus::Completed;
        }
        self.summary.completed_symbols = self
            .progress
            .iter()
            .filter(|p| p.status == SymbolSyncStatus::Completed)
            .count();
    }

    pub fn add_counts(&mut self, inserted: i32, skipped: i32, failed: i32) {
        self.summary.total_inserted += inserted;
        self.summary.total_skipped += skipped;
        self.summary.total_failed += failed;
    }
}

// ── Redis key 工具 ────────────────────────────────────────────────────────────

fn redis_key(sync_id: &str) -> String {
    format!("{}:{}", REDIS_SYNC_KEY_PREFIX, sync_id)
}

fn cancel_key(sync_id: &str) -> String {
    format!("sync_cancel:{}", sync_id)
}

// ── Async Redis 操作 ──────────────────────────────────────────────────────────
// AppState.redis_client 型別為 MultiplexedConnection（已連線），
// 直接 clone() 取得副本使用，不需再 get_connection()。

/// 將 SyncState 寫入 Redis，TTL 24 小時。
pub async fn save_sync_state(
    redis: &mut MultiplexedConnection,
    state: &SyncState,
) -> Result<(), BridgeError> {
    let key = redis_key(&state.sync_id);
    let value = serde_json::to_string(state).map_err(|e| {
        error!(error = %e, "Failed to serialize SyncState");
        BridgeError::internal(format!("SyncState serialize failed: {}", e))
    })?;

    redis
        .set_ex::<_, _, ()>(&key, &value, REDIS_SYNC_TTL_SECS)
        .await
        .map_err(|e| {
            error!(
                error   = %e,
                sync_id = %state.sync_id,
                "Failed to save SyncState to Redis"
            );
            BridgeError::from_cache("save_sync_state failed", e)
        })?;

    Ok(())
}

/// 從 Redis 讀取 SyncState。
pub async fn load_sync_state(
    redis: &mut MultiplexedConnection,
    sync_id: &str,
) -> Result<Option<SyncState>, BridgeError> {
    let key = redis_key(sync_id);
    let raw: Option<String> = redis.get(&key).await.map_err(|e| {
        error!(error = %e, sync_id = %sync_id, "Failed to load SyncState from Redis");
        BridgeError::from_cache("load_sync_state failed", e)
    })?;

    match raw {
        None => Ok(None),
        Some(json) => {
            let state = serde_json::from_str(&json).map_err(|e| {
                error!(
                    error   = %e,
                    sync_id = %sync_id,
                    "Failed to deserialize SyncState"
                );
                BridgeError::internal(format!("SyncState deserialize failed: {}", e))
            })?;
            Ok(Some(state))
        }
    }
}

/// 掃描 Redis，找出進行中的同步任務。
pub async fn find_running_sync(
    redis: &mut MultiplexedConnection,
) -> Result<Option<SyncState>, BridgeError> {
    let pattern = format!("{}:*", REDIS_SYNC_KEY_PREFIX);

    // KEYS 在生產環境會阻塞 Redis，此處資料量小（同時最多 1 個 sync）可接受
    // 若未來 sync 頻繁，改用 SCAN 迭代
    let keys: Vec<String> = redis.keys(&pattern).await.map_err(|e| {
        warn!(error = %e, "Failed to scan Redis for running sync");
        BridgeError::from_cache("find_running_sync keys failed", e)
    })?;

    for key in keys {
        let raw: Option<String> = redis.get(&key).await.unwrap_or(None);
        if let Some(json) = raw {
            if let Ok(state) = serde_json::from_str::<SyncState>(&json) {
                if state.is_in_progress() {
                    return Ok(Some(state));
                }
            }
        }
    }

    Ok(None)
}

/// 設定取消旗標，讓背景同步流程在下一個批次安全停止。
pub async fn request_sync_cancel(
    redis: &mut MultiplexedConnection,
    sync_id: &str,
) -> Result<(), BridgeError> {
    let key = cancel_key(sync_id);
    redis
        .set_ex::<_, _, ()>(&key, "1", REDIS_SYNC_TTL_SECS)
        .await
        .map_err(|e| BridgeError::from_cache("request_sync_cancel failed", e))
}

/// 檢查是否已請求取消。
pub async fn is_sync_cancel_requested(
    redis: &mut MultiplexedConnection,
    sync_id: &str,
) -> Result<bool, BridgeError> {
    let key = cancel_key(sync_id);
    redis
        .exists::<_, bool>(&key)
        .await
        .map_err(|e| BridgeError::from_cache("is_sync_cancel_requested failed", e))
}
