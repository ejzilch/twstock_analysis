/// 手動同步 API handler（修正版）。
///
/// 修正清單：
///   1. 統一使用 Arc<AppState>，移除不存在的 AdminSyncAppState
///   2. Redis 用法改為 state.redis_client.clone()（MultiplexedConnection 可 clone）
///   3. sync_log 操作引用路徑修正
///   4. BridgeError variant 對齊現有定義
///   5. find_running_sync / save_sync_state / load_sync_state 改為 async
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{Local, Months, NaiveDate};
use redis::aio::MultiplexedConnection;
use reqwest::Client;
use sqlx::PgPool;

use std::sync::Arc;

use tracing::{error, info, warn};

use crate::api::handlers::sync_state::{
    find_running_sync, load_sync_state, request_sync_cancel, save_sync_state, SyncState,
};
use crate::api::models::admin_sync::{
    ManualSyncAcceptedResponse, ManualSyncRequest, RateLimitInfo, SyncStatus, SyncStatusResponse,
    SyncSummary,
};
use crate::api::models::SyncMode;
use crate::app_state::AppState;
use crate::constants::{
    ERROR_SYNC_ALREADY_RUNNING, ERROR_SYNC_NOT_FOUND, FINMIND_RATE_LIMIT_BUFFER,
    FINMIND_RATE_LIMIT_PER_HOUR,
};
use crate::core::BridgeError;
use crate::data::db::{sync_log_create, SyncLogEntry};
use crate::data::fetch_rate_limiter::FinMindRateLimiter;
use crate::data::manual_sync::{run_manual_sync, SyncScope};
use crate::data::models::current_timestamp_ms;

// ── POST /api/v1/admin/sync ───────────────────────────────────────────────────

pub async fn trigger_manual_sync(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ManualSyncRequest>,
) -> impl IntoResponse {
    // 驗證請求
    let mode = body.mode.unwrap_or(SyncMode::Partial);

    let target_symbols: Vec<String> = match mode {
        SyncMode::All => match get_all_symbols_from_db(&state.db_pool).await {
            Ok(list) => list,
            Err(e) => {
                error!(error = %e, "Failed to load symbols from DB");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error_code": "INTERNAL_ERROR",
                        "message": "Failed to load symbols",
                        "fallback_available": false,
                        "timestamp_ms": current_timestamp_ms(),
                        "request_id": body.request_id,
                    })),
                )
                    .into_response();
            }
        },
        SyncMode::Partial => {
            let symbols = match &body.symbols {
                Some(s) if !s.is_empty() => s.clone(),
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error_code": "INVALID_REQUEST",
                            "message": "symbols must not be empty",
                            "fallback_available": false,
                            "timestamp_ms": current_timestamp_ms(),
                            "request_id": body.request_id,
                        })),
                    )
                        .into_response();
                }
            };
            symbols
        }
    };

    let symbols = target_symbols;
    let full_sync = body.full_sync.unwrap_or(true);
    let today = Local::now().date_naive();
    let five_years_ago = today
        .checked_sub_months(Months::new(72))
        .expect("日期計算發生錯誤");
    let from_date = if full_sync {
        Some(five_years_ago)
    } else {
        match body
            .from_date
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        {
            Some(d) => Some(d),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error_code": "INVALID_REQUEST",
                        "message": "from_date is required in custom mode (YYYY-MM-DD)",
                        "fallback_available": false,
                        "timestamp_ms": current_timestamp_ms(),
                        "request_id": body.request_id,
                    })),
                )
                    .into_response();
            }
        }
    };

    let to_date = if full_sync {
        Some(today)
    } else {
        match body
            .to_date
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        {
            Some(d) => Some(d),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error_code": "INVALID_REQUEST",
                        "message": "to_date is required in custom mode (YYYY-MM-DD)",
                        "fallback_available": false,
                        "timestamp_ms": current_timestamp_ms(),
                        "request_id": body.request_id,
                    })),
                )
                    .into_response();
            }
        }
    };

    if let (Some(from), Some(to)) = (from_date, to_date) {
        if from > to {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error_code": "INVALID_REQUEST",
                    "message": "from_date must be earlier than or equal to to_date",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": body.request_id,
                })),
            )
                .into_response();
        }
    }

    let scope = SyncScope {
        full_sync,
        from_date,
        to_date,
        intervals: body.intervals.clone().unwrap_or_default(),
    };

    // 檢查是否已有同步執行中
    // MultiplexedConnection::clone() 取得獨立連線副本，不需重新連線
    {
        let mut redis = state.redis_client.clone();
        match find_running_sync(&mut redis).await {
            Ok(Some(running)) => {
                warn!(
                    running_sync_id = %running.sync_id,
                    request_id      = %body.request_id,
                    "Rejected: sync already running"
                );
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error_code": ERROR_SYNC_ALREADY_RUNNING,
                        "message": "A manual sync is already in progress. Check /api/v1/admin/sync/status.",
                        "sync_id": running.sync_id,
                        "fallback_available": false,
                        "timestamp_ms": current_timestamp_ms(),
                        "request_id": body.request_id,
                    })),
                )
                    .into_response();
            }
            Err(e) => {
                // Redis 查詢失敗時寬鬆處理，不因 cache 問題阻擋同步
                warn!(error = %e, "Redis check failed, proceeding with sync");
            }
            Ok(None) => {}
        }
    }

    // 產生 sync_id
    let sync_id = generate_sync_id(&body.request_id);
    let started_at_ms = current_timestamp_ms();

    // 預估請求次數與時間
    let estimated_requests = estimate_requests(&symbols);
    let estimated_hours = estimate_hours(estimated_requests);

    // 建立初始狀態並存入 Redis
    let initial_state = SyncState::new(sync_id.clone(), symbols.clone(), started_at_ms);
    {
        let mut redis = state.redis_client.clone();
        if let Err(e) = save_sync_state(&mut redis, &initial_state).await {
            error!(error = %e, sync_id = %sync_id, "Failed to save initial sync state");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error_code": "INTERNAL_ERROR",
                    "message":    "Failed to initialize sync state",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": body.request_id,
                })),
            )
                .into_response();
        }
    }

    // 寫入 sync_log
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
        // sync_log 寫入失敗不阻擋同步，僅記錄 error
        error!(error = %e, sync_id = %sync_id, "Failed to create sync_log entry");
    }

    // 啟動背景 task（clone 所有需要的資源）
    let db_pool = state.db_pool.clone();
    let http_client = state.http_client.clone();
    let rate_limiter = state.rate_limiter.clone();
    let redis_clone = state.redis_client.clone();
    let sync_id_bg = sync_id.clone();
    let symbols_bg = symbols.clone();
    let scope_bg = scope.clone();

    tokio::spawn(async move {
        run_manual_sync_with_state_tracking(
            db_pool,
            http_client,
            rate_limiter,
            redis_clone,
            sync_id_bg,
            symbols_bg,
            scope_bg,
        )
        .await;
    });

    info!(
        sync_id            = %sync_id,
        symbols            = ?symbols,
        estimated_requests = estimated_requests,
        estimated_hours    = estimated_hours,
        "Manual sync accepted, background task started"
    );

    (
        StatusCode::ACCEPTED,
        Json(
            serde_json::to_value(ManualSyncAcceptedResponse {
                sync_id,
                status: SyncStatus::Running,
                symbols: symbols,
                estimated_requests,
                estimated_hours,
                started_at_ms,
            })
            .unwrap_or_default(),
        ),
    )
        .into_response()
}

// ── GET /api/v1/admin/sync/status ─────────────────────────────────────────────

/// 查詢最近一次同步進度（不需傳 sync_id，自動找進行中的任務）。
pub async fn get_sync_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut redis = state.redis_client.clone();

    let sync_state = match find_running_sync(&mut redis).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error_code": ERROR_SYNC_NOT_FOUND,
                    "message":    "No active sync found. Use POST /api/v1/admin/sync to start one.",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": null,
                })),
            )
                .into_response();
        }
        Err(e) => {
            error!(error = %e, "Failed to find running sync");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error_code": "INTERNAL_ERROR",
                    "message":    "Failed to query sync status",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": null,
                })),
            )
                .into_response();
        }
    };

    build_status_response(&state, sync_state)
        .await
        .into_response()
}

/// 查詢特定 sync_id 的狀態。
pub async fn get_sync_status_by_id(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(sync_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let mut redis = state.redis_client.clone();

    let sync_state = match load_sync_state(&mut redis, &sync_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error_code": ERROR_SYNC_NOT_FOUND,
                    "message":    format!("Sync '{}' not found or expired.", sync_id),
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": null,
                })),
            )
                .into_response();
        }
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "Failed to load sync state");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error_code": "INTERNAL_ERROR",
                    "message":    "Failed to load sync state",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": null,
                })),
            )
                .into_response();
        }
    };

    build_status_response(&state, sync_state)
        .await
        .into_response()
}

/// 取消指定 sync 任務（協作式，於下一批次停止）。
pub async fn cancel_manual_sync(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(sync_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let mut redis = state.redis_client.clone();
    match request_sync_cancel(&mut redis, &sync_id).await {
        Ok(_) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "sync_id": sync_id,
                "status": "cancel_requested",
                "timestamp_ms": current_timestamp_ms(),
            })),
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "Failed to set cancel flag");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error_code": "INTERNAL_ERROR",
                    "message": "Failed to request cancel",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": null,
                })),
            )
                .into_response()
        }
    }
}

/// 查詢 FinMind API quota（不需有進行中的 sync）。
pub async fn get_rate_limit_info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rate_limit = RateLimitInfo {
        used_this_hour: state.rate_limiter.used_this_hour().await,
        limit_per_hour: state.rate_limiter.limit_per_hour(),
        is_waiting: state.rate_limiter.is_waiting().await,
        resume_at_ms: state.rate_limiter.resume_at_ms().await,
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(rate_limit).unwrap_or_default()),
    )
        .into_response()
}

// ── 共用：組裝 SyncStatusResponse ────────────────────────────────────────────

async fn build_status_response(state: &AppState, sync_state: SyncState) -> impl IntoResponse {
    let rate_limit = RateLimitInfo {
        used_this_hour: state.rate_limiter.used_this_hour().await,
        limit_per_hour: state.rate_limiter.limit_per_hour(),
        is_waiting: state.rate_limiter.is_waiting().await,
        resume_at_ms: state.rate_limiter.resume_at_ms().await,
    };

    let response = SyncStatusResponse {
        sync_id: sync_state.sync_id,
        status: sync_state.status,
        started_at_ms: sync_state.started_at_ms,
        rate_limit,
        progress: sync_state.progress,
        summary: sync_state.summary,
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap_or_default()),
    )
}

// ── 背景 task ─────────────────────────────────────────────────────────────────

async fn run_manual_sync_with_state_tracking(
    db_pool: PgPool,
    http_client: Client,
    rate_limiter: Arc<FinMindRateLimiter>,
    mut redis: MultiplexedConnection,
    sync_id: String,
    symbols: Vec<String>,
    scope: SyncScope,
) {
    run_manual_sync(
        db_pool.clone(),
        http_client,
        rate_limiter,
        sync_id.clone(),
        symbols,
        scope,
    )
    .await;

    // 同步結束後從 DB 讀取最終結果，更新 Redis 狀態
    match fetch_final_sync_state(&db_pool, &sync_id).await {
        Ok(Some(final_state)) => {
            if let Err(e) = save_sync_state(&mut redis, &final_state).await {
                error!(error = %e, sync_id = %sync_id, "Failed to save final sync state");
            }
        }
        Ok(None) => warn!(sync_id = %sync_id, "Final sync state not found in DB"),
        Err(e) => error!(error = %e, sync_id = %sync_id, "Failed to fetch final sync state"),
    }
}

/// 從 sync_log 讀取最終狀態，組裝 SyncState。
async fn fetch_final_sync_state(
    db_pool: &PgPool,
    sync_id: &str,
) -> Result<Option<SyncState>, BridgeError> {
    use crate::api::models::admin_sync::{SymbolProgress, SymbolSyncStatus};

    let row = sqlx::query!(
        r#"
        SELECT
            sync_id, symbols,
            total_inserted, total_skipped, total_failed,
            started_at_ms, status
        FROM sync_log
        WHERE sync_id = $1
        "#,
        sync_id,
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

pub async fn get_all_symbols_from_db(db: &PgPool) -> Result<Vec<String>, BridgeError> {
    let rows = sqlx::query!(
        r#"
        SELECT symbol
        FROM symbols
        WHERE is_active = true
        "#
    )
    .fetch_all(db)
    .await
    .map_err(|e| BridgeError::from_db("get_all_symbols failed", e))?;

    Ok(rows.into_iter().map(|r| r.symbol).collect())
}

// ── 工具函數 ──────────────────────────────────────────────────────────────────

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
    // 保守估計：13 年 × 12 月 × 6 粒度
    let avg_gap_months: u64 = 156;
    symbols.len() as u64 * 6 * avg_gap_months
}

fn estimate_hours(estimated_requests: u64) -> u64 {
    let safe_limit = (FINMIND_RATE_LIMIT_PER_HOUR - FINMIND_RATE_LIMIT_BUFFER) as u64;
    estimated_requests.div_ceil(safe_limit)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sync_id_starts_with_sync() {
        let id = generate_sync_id("req-20260419-ABC123");
        assert!(id.starts_with("sync-"));
    }

    #[test]
    fn test_generate_sync_id_has_three_parts() {
        let id = generate_sync_id("req-20260419-ABC123");
        assert_eq!(id.split('-').count(), 3);
    }

    #[test]
    fn test_estimate_requests_one_symbol() {
        let symbols = vec!["2330".to_string()];
        assert_eq!(estimate_requests(&symbols), 6 * 156);
    }

    #[test]
    fn test_estimate_requests_ten_symbols() {
        let symbols: Vec<_> = (0..10).map(|i| format!("{:04}", i)).collect();
        assert_eq!(estimate_requests(&symbols), 10 * 6 * 156);
    }

    #[test]
    fn test_estimate_hours_positive() {
        assert!(estimate_hours(estimate_requests(&["2330".to_string()])) > 0);
    }

    #[test]
    fn test_sync_status_in_progress() {
        assert!(SyncStatus::Running.is_in_progress());
        assert!(SyncStatus::RateLimitWaiting.is_in_progress());
        assert!(!SyncStatus::Completed.is_in_progress());
        assert!(!SyncStatus::Failed.is_in_progress());
    }
}
