use crate::app_state::AppState;
use crate::constants::{ERROR_SYNC_ALREADY_RUNNING, ERROR_SYNC_NOT_FOUND};
use crate::data::models::current_timestamp_ms;
use crate::models::enums::Interval;
/// 手動同步 API handlers — 薄 handler 層
///
/// 職責：parse request → call service → build HTTP response。
/// 所有業務邏輯（日期解析、Redis 狀態、背景 task）已移至 SyncService。
use crate::models::enums::{SyncMode, SyncStatus};
use crate::services::admin_sync::{
    StartSyncRequest, SymbolProgress, SyncService, SyncServiceError, SyncSummary,
};
use crate::services::sync_state::{find_running_sync, load_sync_state, request_sync_cancel};
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

/// Rate limit 即時狀態
#[derive(Debug, Serialize)]
pub struct RateLimitInfo {
    pub used_this_hour: u32,
    pub limit_per_hour: u32,
    pub is_waiting: bool,
    /// 若 is_waiting == true，rate limit 解除的毫秒級 timestamp
    pub resume_at_ms: Option<i64>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct DailyScheduleConfig {
    pub enabled: bool,
    pub time: String,
}

const SCHEDULE_KEY_ENABLED: &str = "daily_sync_enabled";
const SCHEDULE_KEY_TIME: &str = "daily_sync_time";

pub async fn get_daily_schedule(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut redis = state.redis_client.clone();
    let enabled: Option<String> = redis::cmd("GET")
        .arg(SCHEDULE_KEY_ENABLED)
        .query_async(&mut redis)
        .await
        .ok()
        .flatten();
    let time: Option<String> = redis::cmd("GET")
        .arg(SCHEDULE_KEY_TIME)
        .query_async(&mut redis)
        .await
        .ok()
        .flatten();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "enabled": enabled.as_deref() == Some("true"),
            "time": time.unwrap_or_else(|| "02:00".to_string())
        })),
    )
        .into_response()
}

pub async fn update_daily_schedule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DailyScheduleConfig>,
) -> impl IntoResponse {
    let mut redis = state.redis_client.clone();
    let _: redis::RedisResult<()> = redis::cmd("SET")
        .arg(SCHEDULE_KEY_ENABLED)
        .arg(if body.enabled { "true" } else { "false" })
        .query_async(&mut redis)
        .await;
    let _: redis::RedisResult<()> = redis::cmd("SET")
        .arg(SCHEDULE_KEY_TIME)
        .arg(&body.time)
        .query_async(&mut redis)
        .await;
    (
        StatusCode::OK,
        Json(serde_json::json!({"enabled": body.enabled, "time": body.time})),
    )
        .into_response()
}

// ── 共用：組裝 SyncStatusResponse ────────────────────────────────────────────

async fn build_status_response(
    state: &AppState,
    sync_state: crate::services::sync_state::SyncState,
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
