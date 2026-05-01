/// 手動同步 API handlers — 薄 handler 層
///
/// 職責：parse request → call service → build HTTP response。
/// 所有業務邏輯（日期解析、Redis 狀態、背景 task）已移至 SyncService。
use crate::api::handlers::sync_state::{find_running_sync, load_sync_state, request_sync_cancel};
use crate::api::models::SyncMode;

use crate::app_state::AppState;
use crate::constants::{ERROR_SYNC_ALREADY_RUNNING, ERROR_SYNC_NOT_FOUND};
use crate::data::models::current_timestamp_ms;
use crate::models::enums::Interval;
use crate::services::admin_sync::{StartSyncRequest, SyncService, SyncServiceError};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

// ── Request ───────────────────────────────────────────────────────────────────

/// POST /api/v1/admin/sync 請求體
#[derive(Debug, Deserialize)]
pub struct AdminSyncRequest {
    /// 冪等性識別碼，由前端產生
    pub request_id: String,

    /// 冪等性識別碼，由前端產生
    pub mode: Option<SyncMode>,

    /// 要同步的股票代號清單，至少 1 檔
    pub symbols: Option<Vec<String>>,

    /// 是否全量回補（true: 忽略 from/to）
    pub full_sync: Option<bool>,
    /// 自訂起始日期（YYYY-MM-DD）
    pub from_date: Option<String>,
    /// 自訂結束日期（YYYY-MM-DD）
    pub to_date: Option<String>,
    /// 只同步指定 K 線刻度；空值代表全部
    pub intervals: Option<Vec<Interval>>,
}

// ── Response ──────────────────────────────────────────────────────────────────

/// POST /api/v1/admin/sync 成功回應（202）
#[derive(Debug, Serialize)]
pub struct ManualSyncAcceptedResponse {
    pub sync_id: String,
    pub status: SyncStatus,
    pub symbols: Vec<String>,
    pub estimated_requests: u64,
    pub estimated_hours: u64,
    pub started_at_ms: i64,
}

/// GET /api/v1/admin/sync/status 回應
#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    pub sync_id: String,
    pub status: SyncStatus,
    pub started_at_ms: i64,
    pub rate_limit: RateLimitInfo,
    pub progress: Vec<SymbolProgress>,
    pub summary: SyncSummary,
}

/// 同步任務狀態
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Running,
    RateLimitWaiting,
    Completed,
    Failed,
}

impl SyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatus::Running => "running",
            SyncStatus::RateLimitWaiting => "rate_limit_waiting",
            SyncStatus::Completed => "completed",
            SyncStatus::Failed => "failed",
        }
    }

    /// 是否仍在進行中（供前端判斷是否繼續輪詢）
    pub fn is_in_progress(&self) -> bool {
        matches!(self, SyncStatus::Running | SyncStatus::RateLimitWaiting)
    }
}

/// Rate limit 即時狀態
#[derive(Debug, Serialize)]
pub struct RateLimitInfo {
    pub used_this_hour: u32,
    pub limit_per_hour: u32,
    pub is_waiting: bool,
    /// 若 is_waiting == true，rate limit 解除的毫秒級 timestamp
    pub resume_at_ms: Option<i64>,
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

/// 單一股票的同步狀態
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolSyncStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
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

/// 整體同步結果摘要
#[derive(Debug, Serialize, Default, Clone, Deserialize)]
pub struct SyncSummary {
    pub total_symbols: usize,
    pub completed_symbols: usize,
    pub total_inserted: i32,
    pub total_skipped: i32,
    pub total_failed: i32,
}

// ── POST /api/v1/admin/sync ───────────────────────────────────────────────────

pub async fn trigger_manual_sync(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AdminSyncRequest>,
) -> impl IntoResponse {
    let req = StartSyncRequest {
        request_id: body.request_id.clone(),
        mode: body.mode.unwrap_or(SyncMode::Partial),
        symbols: body.symbols.clone(),
        full_sync: body.full_sync.unwrap_or(true),
        from_date: body.from_date.clone(),
        to_date: body.to_date.clone(),
        intervals: body.intervals.clone().unwrap_or_default(),
    };

    match SyncService::start(&state, req).await {
        Ok(accepted) => (
            StatusCode::ACCEPTED,
            Json(serde_json::to_value(accepted).unwrap_or_default()),
        )
            .into_response(),

        Err(SyncServiceError::AlreadyRunning(running_id)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error_code": ERROR_SYNC_ALREADY_RUNNING,
                "message": "A manual sync is already in progress.",
                "sync_id": running_id,
                "fallback_available": false,
                "timestamp_ms": current_timestamp_ms(),
                "request_id": body.request_id,
            })),
        )
            .into_response(),

        Err(SyncServiceError::InvalidRequest(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error_code": "INVALID_REQUEST",
                "message": msg,
                "fallback_available": false,
                "timestamp_ms": current_timestamp_ms(),
                "request_id": body.request_id,
            })),
        )
            .into_response(),

        Err(SyncServiceError::Internal(msg)) => {
            error!(error = %msg, "SyncService internal error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error_code": "INTERNAL_ERROR",
                    "message": "Internal error, please retry",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": body.request_id,
                })),
            )
                .into_response()
        }
    }
}

// ── GET /api/v1/admin/sync/status ─────────────────────────────────────────────

pub async fn get_sync_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut redis = state.redis_client.clone();

    match find_running_sync(&mut redis).await {
        Ok(Some(sync_state)) => build_status_response(&state, sync_state)
            .await
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error_code": ERROR_SYNC_NOT_FOUND,
                "message": "No active sync found.",
                "fallback_available": false,
                "timestamp_ms": current_timestamp_ms(),
                "request_id": null,
            })),
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, "Failed to find running sync");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error_code": "INTERNAL_ERROR",
                    "message": "Failed to query sync status",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": null,
                })),
            )
                .into_response()
        }
    }
}

// ── GET /api/v1/admin/sync/status/:sync_id ────────────────────────────────────

pub async fn get_sync_status_by_id(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(sync_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let mut redis = state.redis_client.clone();

    match load_sync_state(&mut redis, &sync_id).await {
        Ok(Some(sync_state)) => build_status_response(&state, sync_state)
            .await
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error_code": ERROR_SYNC_NOT_FOUND,
                "message": format!("Sync '{}' not found or expired.", sync_id),
                "fallback_available": false,
                "timestamp_ms": current_timestamp_ms(),
                "request_id": null,
            })),
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "Failed to load sync state");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error_code": "INTERNAL_ERROR",
                    "message": "Failed to load sync state",
                    "fallback_available": false,
                    "timestamp_ms": current_timestamp_ms(),
                    "request_id": null,
                })),
            )
                .into_response()
        }
    }
}

// ── POST /api/v1/admin/sync/cancel/:sync_id ───────────────────────────────────

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

// ── GET /api/v1/admin/sync/rate-limit ─────────────────────────────────────────

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

async fn build_status_response(
    state: &AppState,
    sync_state: crate::api::handlers::sync_state::SyncState,
) -> impl IntoResponse {
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
