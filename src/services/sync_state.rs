/// 同步任務狀態的 Redis 存取層。
///
/// 改用 redis::aio::MultiplexedConnection（async），
/// 對應 AppState 中的 redis_client 型別。
use crate::constants::{REDIS_SYNC_KEY_PREFIX, REDIS_SYNC_TTL_SECS};
use crate::domain::BridgeError;
use crate::models::enums::{SymbolSyncStatus, SyncStatus};
use crate::services::admin_sync::SyncSummary;

use super::sync_types::{GapProgress, SymbolProgress};

use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

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

    pub fn _update_gap_a(&mut self, symbol: &str, gap: GapProgress) {
        if let Some(p) = self.progress.iter_mut().find(|p| p.symbol == symbol) {
            p.gap_a = Some(gap);
        }
    }

    pub fn _update_gap_b(&mut self, symbol: &str, gap: GapProgress) {
        if let Some(p) = self.progress.iter_mut().find(|p| p.symbol == symbol) {
            p.gap_b = Some(gap);
        }
    }

    pub fn _mark_symbol_completed(&mut self, symbol: &str) {
        if let Some(p) = self.progress.iter_mut().find(|p| p.symbol == symbol) {
            p.status = SymbolSyncStatus::Completed;
        }
        self.summary.completed_symbols = self
            .progress
            .iter()
            .filter(|p| p.status == SymbolSyncStatus::Completed)
            .count();
    }

    pub fn _add_counts(&mut self, inserted: i32, skipped: i32, failed: i32) {
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

pub async fn update_sync_status(
    redis: &mut MultiplexedConnection,
    sync_id: &str,
    status: SyncStatus,
) -> Result<(), BridgeError> {
    if let Some(mut state) = load_sync_state(redis, sync_id).await? {
        state.status = status;
        save_sync_state(redis, &state).await?;
    }
    Ok(())
}

// ── 新增：逐 symbol 進度更新 ──────────────────────────────────────────────────

/// 更新單一 symbol 的同步進度並回寫 Redis。
///
/// 每個 symbol 的所有 gap 處理完後呼叫一次，不是每個 batch 呼叫，
/// 避免過於頻繁的 Redis 寫入。
///
/// # Arguments
/// * `symbol`  - 股票代號
/// * `status`  - 該 symbol 的新狀態（running / completed / failed）
/// * `gap_a`   - 歷史段結果，None 表示無此缺口
/// * `gap_b`   - 近期段結果，None 表示無此缺口
pub async fn update_symbol_progress(
    redis: &mut MultiplexedConnection,
    sync_id: &str,
    symbol: &str,
    status: SymbolSyncStatus,
    gap_a: Option<GapProgress>,
    gap_b: Option<GapProgress>,
) -> Result<(), BridgeError> {
    let mut state = match load_sync_state(redis, sync_id).await? {
        Some(s) => s,
        None => {
            warn!(sync_id = %sync_id, symbol = %symbol, "SyncState not found in Redis, skip update");
            return Ok(());
        }
    };

    if let Some(p) = state.progress.iter_mut().find(|p| p.symbol == symbol) {
        p.status = status.clone();
        if gap_a.is_some() {
            p.gap_a = gap_a;
        }
        if gap_b.is_some() {
            p.gap_b = gap_b;
        }
    }

    // 同步更新 summary 的 completed_symbols 計數
    state.summary.completed_symbols = state
        .progress
        .iter()
        .filter(|p| {
            matches!(
                p.status,
                SymbolSyncStatus::Completed | SymbolSyncStatus::Failed | SymbolSyncStatus::Skipped
            )
        })
        .count();

    save_sync_state(redis, &state).await
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

/// 強制清除同步狀態與取消旗標。
pub async fn clear_sync_state(
    redis: &mut MultiplexedConnection,
    sync_id: &str,
) -> Result<(), BridgeError> {
    let state_key = redis_key(sync_id);
    let c_key = cancel_key(sync_id);
    redis
        .del::<_, ()>(&state_key)
        .await
        .map_err(|e| BridgeError::from_cache("clear_sync_state(state) failed", e))?;
    redis
        .del::<_, ()>(&c_key)
        .await
        .map_err(|e| BridgeError::from_cache("clear_sync_state(cancel) failed", e))?;
    Ok(())
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
