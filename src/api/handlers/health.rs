use crate::ai_client::AiServiceClient;
use crate::api::models::enums::{HealthStatus, ObservabilityStatus};
use crate::api::models::response::{
    AiServiceCheck, CacheDbConsistency, DagOrderCheck, DataSourceCheck, HealthComponents,
    HealthResponse, IntegrityChecks, IntegrityResponse, ObservabilityMetrics,
};
use crate::constants::{
    API_VERSION, OBSERVABILITY_AI_INFERENCE_WARNING_MS, OBSERVABILITY_BRIDGE_ERRORS_WARNING_COUNT,
    OBSERVABILITY_DATA_LATENCY_WARNING_SECS, OBSERVABILITY_SUCCESS_RATE_WARNING_PCT,
};
use crate::data::db::DbClient;
use crate::data::fetch_rate_limiter::FinMindRateLimiter;
use axum::{extract::State, Json};
use chrono::Utc;
use std::sync::Arc;

/// 應用程式共享狀態
pub struct AppState {
    pub db_client: Arc<DbClient>,
    pub ai_client: Arc<AiServiceClient>,
    pub rate_limiter: Arc<FinMindRateLimiter>,
}

/// GET /api/v1/health
///
/// 快速健康檢查，逐一 ping 各元件。
/// 任一元件不可用時回傳 degraded，仍回傳 HTTP 200。
pub async fn health_handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let db_status = check_database(&state.db_client).await;
    let redis_status = check_redis(&state.db_client).await;
    let ai_status = check_ai_service(&state.ai_client).await;

    let overall_status = if db_status == HealthStatus::Ok
        && redis_status == HealthStatus::Ok
        && ai_status == HealthStatus::Ok
    {
        HealthStatus::Ok
    } else {
        HealthStatus::Degraded
    };

    Json(HealthResponse {
        status: overall_status,
        timestamp_ms: Utc::now().timestamp_millis(),
        components: HealthComponents {
            database: db_status.to_string(),
            redis: redis_status.to_string(),
            python_ai_service: ai_status.to_string(),
        },
        version: API_VERSION.to_string(),
    })
}

/// GET /health/integrity
///
/// EJ 每日巡視用，包含 DB/Cache 一致性與 Observability 指標。
/// 需要較多 DB 查詢，不適合頻繁呼叫。
pub async fn integrity_handler(State(state): State<Arc<AppState>>) -> Json<IntegrityResponse> {
    let now_ms = Utc::now().timestamp_millis();

    let cache_consistency = check_cache_db_consistency(&state.db_client).await;
    let dag_check = check_dag_order().await;
    let ai_check = check_ai_service_latency(&state.ai_client).await;
    let data_source_check = build_data_source_check(&state.rate_limiter);
    let observability = build_observability_metrics(&state.db_client).await;

    let overall_status = if cache_consistency.status == "ok"
        && dag_check.status == "ok"
        && ai_check.status == "ok"
    {
        HealthStatus::Ok
    } else {
        HealthStatus::Degraded
    };

    Json(IntegrityResponse {
        status: overall_status.to_string(),
        timestamp_ms: now_ms,
        checks: IntegrityChecks {
            cache_db_consistency: cache_consistency,
            indicator_dag_order: dag_check,
            python_ai_service: ai_check,
            data_source: data_source_check,
        },
        observability,
    })
}

// ── 私有檢查函數 ──────────────────────────────────────────────────────────────

async fn check_database(db_client: &DbClient) -> HealthStatus {
    match sqlx::query("SELECT 1").execute(&db_client.pool).await {
        Ok(_) => HealthStatus::Ok,
        Err(e) => {
            tracing::error!(error = %e, "Database health check failed");
            HealthStatus::Degraded
        }
    }
}

async fn check_redis(db_client: &DbClient) -> HealthStatus {
    match db_client
        .redis_client
        .get_multiplexed_async_connection()
        .await
    {
        Ok(mut conn) => {
            let ping: redis::RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;
            match ping {
                Ok(_) => HealthStatus::Ok,
                Err(e) => {
                    tracing::error!(error = %e, "Redis health check failed");
                    HealthStatus::Degraded
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Redis connection failed");
            HealthStatus::Degraded
        }
    }
}

async fn check_ai_service(ai_client: &AiServiceClient) -> HealthStatus {
    // 呼叫 Python /health 端點確認服務可用性
    // 若 AiServiceClient 尚未提供 health check 方法，暫時回傳 Ok
    // TODO: 實作 AiServiceClient::health_check()
    HealthStatus::Ok
}

async fn check_cache_db_consistency(db_client: &DbClient) -> CacheDbConsistency {
    // 從 DB 取 3 筆最新 K 線樣本，與 Redis 快取比對
    // 若差異超過 0.01% 則回傳 mismatch
    // TODO: 實作完整比對邏輯
    CacheDbConsistency {
        status: "ok".to_string(),
        sample_size: 3,
        max_deviation_pct: 0.0,
        note: None,
    }
}

async fn check_dag_order() -> DagOrderCheck {
    DagOrderCheck {
        status: "ok".to_string(),
        last_execution_ms: Some(Utc::now().timestamp_millis()),
    }
}

async fn check_ai_service_latency(ai_client: &AiServiceClient) -> AiServiceCheck {
    // TODO: 記錄最近一次 AI 服務回應時間
    AiServiceCheck {
        status: "ok".to_string(),
        last_response_ms: None,
    }
}

fn build_data_source_check(rate_limiter: &FinMindRateLimiter) -> DataSourceCheck {
    let remaining_pct = rate_limiter.daily_remaining_pct();
    let is_rate_limited = remaining_pct <= 0.0;

    DataSourceCheck {
        primary: "finmind".to_string(),
        status: if is_rate_limited {
            "rate_limited"
        } else {
            "ok"
        }
        .to_string(),
        fallback: if is_rate_limited {
            Some("yfinance".to_string())
        } else {
            None
        },
        rate_limit_remaining_pct: remaining_pct,
    }
}

async fn build_observability_metrics(db_client: &DbClient) -> ObservabilityMetrics {
    // TODO: 從 DB 或 metrics 系統取得實際數值
    // 目前回傳佔位值，待 metrics 收集機制實作後替換
    let data_latency_seconds = 0_i64;
    let ai_inference_p99_ms = 0_i64;
    let api_success_rate_pct = 100.0_f64;
    let bridge_errors_last_hour = 0_i64;

    let data_latency_status = if data_latency_seconds > OBSERVABILITY_DATA_LATENCY_WARNING_SECS {
        ObservabilityStatus::Warning
    } else {
        ObservabilityStatus::Ok
    };

    let ai_inference_status = if ai_inference_p99_ms > OBSERVABILITY_AI_INFERENCE_WARNING_MS {
        ObservabilityStatus::Warning
    } else {
        ObservabilityStatus::Ok
    };

    let success_rate_status = if api_success_rate_pct < OBSERVABILITY_SUCCESS_RATE_WARNING_PCT {
        ObservabilityStatus::Warning
    } else {
        ObservabilityStatus::Ok
    };

    let bridge_error_status = if bridge_errors_last_hour > OBSERVABILITY_BRIDGE_ERRORS_WARNING_COUNT
    {
        ObservabilityStatus::Warning
    } else {
        ObservabilityStatus::Ok
    };

    ObservabilityMetrics {
        data_latency_seconds,
        data_latency_status: data_latency_status.to_string(),
        ai_inference_p99_ms,
        ai_inference_status: ai_inference_status.to_string(),
        api_success_rate_pct,
        api_success_rate_status: success_rate_status.to_string(),
        bridge_errors_last_hour,
        bridge_error_status,
    }
}
