use crate::api::models::{FetchSource, HealthStatus, ReliabilityLevel, SignalSource};
use crate::models::{IndicatorValue, Interval, SignalType};
use serde::Serialize;
use std::collections::HashMap;

// ── K 線 Response ─────────────────────────────────────────────────────────────

/// GET /api/v1/candles/{symbol} 的單筆 K 線資料
///
/// 從 Candle domain model 轉換而來，不含 symbol 與 interval（由外層 response 提供）。
#[derive(Debug, Clone, Serialize)]
pub struct CandleResponse {
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    /// 動態指標結果，key 為指標名稱（如 "ma20"、"rsi14"、"macd"）
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub indicators: HashMap<String, IndicatorValue>,
}

/// GET /api/v1/candles/{symbol} 的完整 response
#[derive(Debug, Clone, Serialize)]
pub struct CandlesApiResponse {
    pub symbol: String,
    pub interval: Interval,
    pub from_ms: i64,
    pub to_ms: i64,
    pub candles: Vec<CandleResponse>,
    pub count: usize,
    pub total_available: usize,
    /// 分頁游標，None 表示已取完全部資料
    pub next_cursor: Option<String>,
    /// 資料來源："database" / "cache"
    pub source: FetchSource,
    pub cached: bool,
    pub computed_at_ms: i64,
}

/// GET /api/v1/signals/{symbol} 的單筆信號資料
#[derive(Debug, Clone, Serialize)]
pub struct TradeSignalResponse {
    pub id: String,
    pub timestamp_ms: i64,
    /// "BUY" / "SELL"
    pub signal_type: SignalType,
    pub confidence: f64,
    pub entry_price: f64,
    pub target_price: f64,
    pub stop_loss: f64,
    pub reason: String,
    /// AiEnsemble / TechnicalOnly / ManualOverride,
    pub source: SignalSource,
    /// High / Medium / Low / Unknown
    pub reliability: ReliabilityLevel,
    /// AI 降級時的原因，如 "AI_SERVICE_TIMEOUT"
    pub fallback_reason: Option<String>,
}

/// GET /api/v1/signals/{symbol} 的完整 response
#[derive(Debug, Clone, Serialize)]
pub struct SignalsApiResponse {
    pub symbol: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub signals: Vec<TradeSignalResponse>,
    pub count: usize,
}

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
    pub data_latency_seconds: i64,
    pub data_latency_status: String,
    pub ai_inference_p99_ms: i64,
    pub ai_inference_status: String,
    pub api_success_rate_pct: f64,
    pub api_success_rate_status: String,
    pub bridge_errors_last_hour: i64,
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
    pub data_source: DataSourceCheck,
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

#[derive(Debug, Clone, Serialize)]
pub struct DataSourceCheck {
    pub primary: String,
    pub status: String,
    pub fallback: Option<String>,
    pub rate_limit_remaining_pct: f64,
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reliability_from_confidence_high() {
        assert!(matches!(
            ReliabilityLevel::from_confidence(0.75),
            ReliabilityLevel::High
        ));
    }

    #[test]
    fn test_reliability_from_confidence_medium() {
        assert!(matches!(
            ReliabilityLevel::from_confidence(0.6),
            ReliabilityLevel::Medium
        ));
    }

    #[test]
    fn test_reliability_from_confidence_low() {
        assert!(matches!(
            ReliabilityLevel::from_confidence(0.3),
            ReliabilityLevel::Low
        ));
    }

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
