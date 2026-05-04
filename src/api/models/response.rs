use crate::api::models::enums::ObservabilityStatus;
use crate::api::models::HealthStatus;
use serde::Serialize;

// ── 錯誤 Response ─────────────────────────────────────────────────────────────

/// 全域錯誤 response，對應 API_CONTRACT.md 的 ErrorResponse 格式
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error_code: String,
    pub message: String,
    pub fallback_available: bool,
    pub timestamp_ms: i64,
    pub request_id: Option<String>,
}

impl ErrorResponse {
    /// 建立無 fallback 的錯誤 response
    pub fn new(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_code: error_code.into(),
            message: message.into(),
            fallback_available: false,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            request_id: None,
        }
    }

    /// 建立有 fallback 的錯誤 response（AI 降級場景）
    pub fn with_fallback(
        error_code: impl Into<String>,
        message: impl Into<String>,
        request_id: Option<String>,
    ) -> Self {
        Self {
            error_code: error_code.into(),
            message: message.into(),
            fallback_available: true,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            request_id,
        }
    }
}

// ── 健康檢查 Response ─────────────────────────────────────────────────────────

/// GET /api/v1/health 的 response
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    /// "ok" / "degraded"
    pub status: HealthStatus,
    pub timestamp_ms: i64,
    pub components: HealthComponents,
    pub version: String,
}

/// 各元件健康狀態
#[derive(Debug, Clone, Serialize)]
pub struct HealthComponents {
    pub database: String,
    pub redis: String,
    pub python_ai_service: String,
}

/// GET /health/integrity 的 Observability 指標
#[derive(Debug, Clone, Serialize)]
pub struct ObservabilityMetrics {
    pub data_latency_seconds: u64,
    pub data_latency_status: String,
    pub ai_inference_p99_ms: u64,
    pub ai_inference_status: String,
    pub api_success_rate_pct: f64,
    pub api_success_rate_status: String,
    pub bridge_errors_last_hour: u32,
    pub bridge_error_status: ObservabilityStatus,
}

/// GET /health/integrity 的完整 response
#[derive(Debug, Clone, Serialize)]
pub struct IntegrityResponse {
    pub status: String,
    pub timestamp_ms: i64,
    pub checks: IntegrityChecks,
    pub observability: ObservabilityMetrics,
}

/// /health/integrity 各項檢查結果
#[derive(Debug, Clone, Serialize)]
pub struct IntegrityChecks {
    pub cache_db_consistency: CacheDbConsistency,
    pub indicator_dag_order: DagOrderCheck,
    pub python_ai_service: AiServiceCheck,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheDbConsistency {
    pub status: String,
    pub sample_size: usize,
    pub max_deviation_pct: f64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DagOrderCheck {
    pub status: String,
    pub last_execution_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiServiceCheck {
    pub status: String,
    pub last_response_ms: Option<i64>,
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response_new_has_no_fallback() {
        let err = ErrorResponse::new("SYMBOL_NOT_FOUND", "Symbol not found");
        assert!(!err.fallback_available);
        assert_eq!(err.error_code, "SYMBOL_NOT_FOUND");
    }

    #[test]
    fn test_error_response_with_fallback() {
        let err = ErrorResponse::with_fallback(
            "AI_SERVICE_TIMEOUT",
            "AI timed out",
            Some("req-001".to_string()),
        );
        assert!(err.fallback_available);
        assert_eq!(err.request_id, Some("req-001".to_string()));
    }
}
