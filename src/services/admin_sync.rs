use crate::app_state::AppState;
use crate::constants::{FINMIND_RATE_LIMIT_BUFFER, FINMIND_RATE_LIMIT_PER_HOUR};
use crate::data::db::{sync_log_create, SyncLogEntry};
/// SyncService — 手動同步業務流程協調者（Service layer）
///
/// 職責：
///   1. 驗證同步請求（date range、symbols）
///   2. 檢查 Redis 是否已有同步進行中
///   3. 建立 SyncState、寫入 sync_log
///   4. 啟動背景 task，追蹤最終狀態
///   5. 組裝 StartSyncResult
///
/// admin_sync handler 只做 parse → call service → return response。
use crate::data::fetch_rate_limiter::FinMindRateLimiter;
use crate::data::manual_sync::{run_manual_sync, SyncScope};
use crate::data::models::current_timestamp_ms;
use crate::domain::BridgeError;
use crate::models::enums::{SymbolSyncStatus, SyncMode, SyncStatus};
use crate::BulkInsertBuffer;
use chrono::{Local, Months, NaiveDate};
use redis::aio::MultiplexedConnection;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::sync_state::{find_running_sync, save_sync_state, SyncState};

// ── 請求 DTO ──────────────────────────────────────────────────────────────────

/// SyncService::start() 的輸入，從 handler 解析完畢後傳入
pub struct StartSyncRequest {
    pub request_id: String,
    pub mode: SyncMode,
    pub symbols: Option<Vec<String>>,
    pub full_sync: bool,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub intervals: Vec<crate::models::enums::Interval>,
}

#[derive(serde::Serialize)]
pub struct StartSyncResult {
    pub sync_id: String,
    pub symbols: Vec<String>,
    pub status: SyncStatus,
    pub estimated_requests: u64,
    pub estimated_hours: u64,
    pub started_at_ms: i64,
}

/// 整體同步結果摘要
#[derive(Debug, Serialize, Default, Clone, Deserialize)]
pub struct SyncSummary {
    pub total_symbols: usize,
    pub completed_symbols: usize,
    pub total_inserted: i32,
    pub total_skipped: i32,
    pub total_failed: i32,
}

/// 單一股票的同步進度
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct SymbolProgress {
    pub symbol: String,
    pub name: String,
    pub status: SymbolSyncStatus,
    /// 缺口 A（歷史段）進度，None 表示無此缺口
    pub gap_a: Option<GapProgress>,
    /// 缺口 B（近期段）進度，None 表示無此缺口
    pub gap_b: Option<GapProgress>,
}

/// 單一缺口的進度
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct GapProgress {
    pub from_ms: i64,
    pub to_ms: i64,
    pub inserted: i32,
    pub skipped: i32,
    pub failed: i32,
    pub completed: bool,
}

// ── Service 入口 ──────────────────────────────────────────────────────────────

pub struct SyncService;

impl SyncService {
    /// 啟動手動同步：驗證 → 建立狀態 → 啟動背景 task。
    pub async fn start(
        state: &AppState,
        req: StartSyncRequest,
    ) -> Result<StartSyncResult, SyncServiceError> {
        // ── Step 1: 解析目標 symbols ──────────────────────────────────────────
        let symbols = Self::resolve_symbols(state, &req).await?;

        // ── Step 2: 解析日期範圍 ──────────────────────────────────────────────
        let (from_date, to_date) = Self::resolve_date_range(&req)?;

        // 驗證日期順序
        if let (Some(from), Some(to)) = (from_date.as_ref(), to_date.as_ref()) {
            if from > to {
                return Err(SyncServiceError::InvalidRequest(
                    "from_date must be earlier than or equal to to_date".to_string(),
                ));
            }
        }

        let scope = SyncScope {
            full_sync: req.full_sync,
            from_date,
            to_date,
            intervals: req.intervals.clone(),
        };

        // ── Step 3: 確認無其他同步進行中 ──────────────────────────────────────
        {
            let mut redis = state.redis_client.clone();
            match find_running_sync(&mut redis).await {
                Ok(Some(running)) => {
                    warn!(
                        running_sync_id = %running.sync_id,
                        "Rejected: sync already running"
                    );
                    return Err(SyncServiceError::AlreadyRunning(running.sync_id));
                }
                Err(e) => warn!(error = %e, "Redis check failed, proceeding with sync"),
                Ok(None) => {}
            }
        }

        // ── Step 4: 產生 sync_id 與初始狀態 ──────────────────────────────────
        let sync_id = Self::generate_sync_id(&req.request_id);
        let started_at_ms = current_timestamp_ms();
        let estimated_requests = Self::estimate_requests(&symbols);
        let estimated_hours = Self::estimate_hours(estimated_requests);

        let initial_state = SyncState::new(sync_id.clone(), symbols.clone(), started_at_ms);
        {
            let mut redis = state.redis_client.clone();
            save_sync_state(&mut redis, &initial_state)
                .await
                .map_err(|e| {
                    error!(error = %e, sync_id = %sync_id, "Failed to save initial sync state");
                    SyncServiceError::Internal("Failed to initialize sync state".to_string())
                })?;
        }

        // ── Step 5: 寫入 sync_log（失敗不阻擋）──────────────────────────────
        if let Err(e) = sync_log_create(
            &state.db_pool,
            &SyncLogEntry {
                sync_id: sync_id.clone(),
                sync_type: "manual".to_string(),
                triggered_by: "ej".to_string(),
                symbols: symbols.clone(),
            },
            started_at_ms,
        )
        .await
        {
            error!(error = %e, sync_id = %sync_id, "Failed to create sync_log entry");
        }

        // ── Step 6: 啟動背景 task ─────────────────────────────────────────────
        let db_pool = state.db_pool.clone();
        let http_client = state.http_client.clone();
        let rate_limiter = state.rate_limiter.clone();
        let redis_clone = state.redis_client.clone();

        tokio::spawn(Self::run_with_state_tracking(
            db_pool,
            http_client,
            rate_limiter,
            redis_clone,
            sync_id.clone(),
            symbols.clone(),
            scope,
            state.bulk_insert_buffer.clone(),
        ));

        info!(
            sync_id            = %sync_id,
            symbols_count      = symbols.len(),
            estimated_requests = estimated_requests,
            estimated_hours    = estimated_hours,
            "Manual sync accepted, background task started"
        );

        Ok(StartSyncResult {
            sync_id,
            status: SyncStatus::Running,
            symbols,
            estimated_requests,
            estimated_hours,
            started_at_ms,
        })
    }

    // ── 私有：業務邏輯 ────────────────────────────────────────────────────────

    async fn resolve_symbols(
        state: &AppState,
        req: &StartSyncRequest,
    ) -> Result<Vec<String>, SyncServiceError> {
        match req.mode {
            SyncMode::All => get_all_active_symbols(&state.db_pool).await.map_err(|e| {
                error!(error = %e, "Failed to load symbols from DB");
                SyncServiceError::Internal("Failed to load symbols".to_string())
            }),
            SyncMode::Partial => match &req.symbols {
                Some(s) if !s.is_empty() => Ok(s.clone()),
                _ => Err(SyncServiceError::InvalidRequest(
                    "symbols must not be empty in partial mode".to_string(),
                )),
            },
        }
    }

    fn resolve_date_range(
        req: &StartSyncRequest,
    ) -> Result<(Option<NaiveDate>, Option<NaiveDate>), SyncServiceError> {
        if req.full_sync {
            let today = Local::now().date_naive();
            let five_years_ago = today
                .checked_sub_months(Months::new(72))
                .expect("Date calculation error");
            return Ok((Some(five_years_ago), Some(today)));
        }

        let from = req
            .from_date
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .ok_or_else(|| {
                SyncServiceError::InvalidRequest(
                    "from_date is required in custom mode (YYYY-MM-DD)".to_string(),
                )
            })?;

        let to = req
            .to_date
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .ok_or_else(|| {
                SyncServiceError::InvalidRequest(
                    "to_date is required in custom mode (YYYY-MM-DD)".to_string(),
                )
            })?;

        Ok((Some(from), Some(to)))
    }

    fn generate_sync_id(request_id: &str) -> String {
        let date = chrono::Utc::now().format("%Y%m%d").to_string();
        let suffix = request_id
            .chars()
            .rev()
            .take(6)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
            .to_uppercase();
        format!("sync-{}-{}", date, suffix)
    }

    fn estimate_requests(symbols: &[String]) -> u64 {
        let avg_gap_months: u64 = 156;
        symbols.len() as u64 * 6 * avg_gap_months
    }

    fn estimate_hours(estimated_requests: u64) -> u64 {
        let safe_limit = (FINMIND_RATE_LIMIT_PER_HOUR - FINMIND_RATE_LIMIT_BUFFER) as u64;
        estimated_requests.div_ceil(safe_limit)
    }

    // ── 背景 task ─────────────────────────────────────────────────────────────

    async fn run_with_state_tracking(
        db_pool: PgPool,
        http_client: Client,
        rate_limiter: Arc<FinMindRateLimiter>,
        mut redis: MultiplexedConnection,
        sync_id: String,
        symbols: Vec<String>,
        scope: SyncScope,
        buffer: Arc<tokio::sync::Mutex<BulkInsertBuffer>>,
    ) {
        // redis.clone() 取得副本傳給 run_manual_sync，
        // 讓它在整個同步過程中使用 AppState 已建立的連線，不自行重建。
        run_manual_sync(
            db_pool.clone(),
            http_client,
            rate_limiter,
            redis.clone(), // ← 傳入現有連線
            sync_id.clone(),
            symbols,
            scope,
            buffer,
        )
        .await;

        // 同步結束後從 DB 讀取最終狀態，更新 Redis
        match fetch_final_sync_state_from_db(&db_pool, &sync_id).await {
            Ok(Some(final_state)) => {
                if let Err(e) = save_sync_state(&mut redis, &final_state).await {
                    error!(error = %e, sync_id = %sync_id, "Failed to save final sync state");
                }
            }
            Ok(None) => warn!(sync_id = %sync_id, "Final sync state not found in DB"),
            Err(e) => error!(error = %e, sync_id = %sync_id, "Failed to fetch final sync state"),
        }
    }
}

// ── 錯誤類型 ──────────────────────────────────────────────────────────────────

/// SyncService 特有的錯誤，由 handler 轉換為對應的 HTTP response
#[derive(Debug)]
pub enum SyncServiceError {
    AlreadyRunning(String), // → handler 回傳 409
    InvalidRequest(String), // → handler 回傳 400
    Internal(String),       // → handler 回傳 500
}

// ── 共用查詢函數 ──────────────────────────────────────────────────────────────

pub async fn get_all_active_symbols(db: &PgPool) -> Result<Vec<String>, BridgeError> {
    let rows = sqlx::query!(r#"SELECT symbol FROM symbols WHERE is_active = true"#)
        .fetch_all(db)
        .await
        .map_err(|e| BridgeError::from_db("get_all_active_symbols failed", e))?;

    Ok(rows.into_iter().map(|r| r.symbol).collect())
}

/// 從 sync_log 讀取最終狀態，組裝 SyncState（供背景 task 回寫 Redis 用）
async fn fetch_final_sync_state_from_db(
    db_pool: &PgPool,
    sync_id: &str,
) -> Result<Option<SyncState>, BridgeError> {
    use crate::models::enums::SymbolSyncStatus;
    use crate::services::admin_sync::SyncSummary;

    let row = sqlx::query!(
        r#"
        SELECT sync_id, symbols,
               total_inserted, total_skipped, total_failed,
               started_at_ms, status
        FROM sync_log WHERE sync_id = $1
        "#,
        sync_id
    )
    .fetch_optional(db_pool)
    .await
    .map_err(|e| BridgeError::from_db("fetch_final_sync_state failed", e))?;

    let row = match row {
        None => return Ok(None),
        Some(r) => r,
    };

    let status = match row.status.as_str() {
        "running" => SyncStatus::Running,
        "rate_limit_waiting" => SyncStatus::RateLimitWaiting,
        "completed" => SyncStatus::Completed,
        _ => SyncStatus::Failed,
    };

    let symbols = row.symbols;
    let total = symbols.len();
    let symbol_status = if status == SyncStatus::Completed {
        SymbolSyncStatus::Completed
    } else {
        SymbolSyncStatus::Pending
    };

    let progress: Vec<SymbolProgress> = symbols
        .iter()
        .map(|s| SymbolProgress {
            symbol: s.clone(),
            name: String::new(),
            status: symbol_status.clone(),
            gap_a: None,
            gap_b: None,
        })
        .collect();

    Ok(Some(SyncState {
        sync_id: row.sync_id,
        status: status.clone(),
        started_at_ms: row.started_at_ms,
        symbols,
        progress,
        summary: SyncSummary {
            total_symbols: total,
            completed_symbols: if status == SyncStatus::Completed {
                total
            } else {
                0
            },
            total_inserted: row.total_inserted,
            total_skipped: row.total_skipped,
            total_failed: row.total_failed,
        },
    }))
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sync_id_format() {
        let id = SyncService::generate_sync_id("req-20260419-ABC123");
        assert!(id.starts_with("sync-"));
        assert_eq!(id.split('-').count(), 3);
    }

    #[test]
    fn test_estimate_requests_one_symbol() {
        let symbols = vec!["2330".to_string()];
        assert_eq!(SyncService::estimate_requests(&symbols), 6 * 156);
    }

    #[test]
    fn test_estimate_requests_ten_symbols() {
        let symbols: Vec<_> = (0..10).map(|i| format!("{:04}", i)).collect();
        assert_eq!(SyncService::estimate_requests(&symbols), 10 * 6 * 156);
    }

    #[test]
    fn test_estimate_hours_positive() {
        let symbols = vec!["2330".to_string()];
        assert!(SyncService::estimate_hours(SyncService::estimate_requests(&symbols)) > 0);
    }

    #[test]
    fn test_resolve_date_range_full_sync() {
        let req = StartSyncRequest {
            request_id: "r".to_string(),
            mode: SyncMode::All,
            symbols: None,
            full_sync: true,
            from_date: None,
            to_date: None,
            intervals: vec![],
        };
        let (from, to) = SyncService::resolve_date_range(&req).unwrap();
        assert!(from.is_some());
        assert!(to.is_some());
        assert!(from.unwrap() < to.unwrap());
    }

    #[test]
    fn test_resolve_date_range_custom_missing_from_returns_error() {
        let req = StartSyncRequest {
            request_id: "r".to_string(),
            mode: SyncMode::Partial,
            symbols: Some(vec!["2330".to_string()]),
            full_sync: false,
            from_date: None,
            to_date: Some("2026-01-31".to_string()),
            intervals: vec![],
        };
        assert!(matches!(
            SyncService::resolve_date_range(&req),
            Err(SyncServiceError::InvalidRequest(_))
        ));
    }

    #[test]
    fn test_resolve_date_range_custom_missing_to_returns_error() {
        let req = StartSyncRequest {
            request_id: "r".to_string(),
            mode: SyncMode::Partial,
            symbols: Some(vec!["2330".to_string()]),
            full_sync: false,
            from_date: Some("2026-01-01".to_string()),
            to_date: None,
            intervals: vec![],
        };
        assert!(matches!(
            SyncService::resolve_date_range(&req),
            Err(SyncServiceError::InvalidRequest(_))
        ));
    }
}
