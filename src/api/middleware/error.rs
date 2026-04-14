use crate::api::models::ErrorResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

/// API 錯誤類型，對應 API_CONTRACT.md 的全域錯誤碼
///
/// 由各 handler 回傳，經 IntoResponse 轉換為標準 HTTP 錯誤格式。
/// 每個 variant 包含對應的 HTTP status code 與 error_code。
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Symbol not found: {symbol}")]
    SymbolNotFound { symbol: String },

    #[error("Query range too large: requested {requested} candles, max is {max}")]
    QueryRangeTooLarge { requested: usize, max: usize },

    #[error("Invalid indicator config: {detail}")]
    InvalidIndicatorConfig { detail: String },

    #[error("Indicator compute failed: {detail}")]
    IndicatorComputeFailed { detail: String },

    #[error("Data source interrupted")]
    DataSourceInterrupted,

    #[error("Data source rate limited, falling back")]
    DataSourceRateLimited,

    #[error("Computation overflow detected")]
    ComputationOverflow,

    #[error("AI service timeout, falling back to technical indicators")]
    AiServiceTimeout,

    #[error("AI service unavailable")]
    AiServiceUnavailable,

    /// 未預期的內部錯誤，不對外暴露細節
    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    /// 回傳對應的 HTTP status code
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::SymbolNotFound { .. } => StatusCode::NOT_FOUND,
            ApiError::QueryRangeTooLarge { .. } => StatusCode::BAD_REQUEST,
            ApiError::InvalidIndicatorConfig { .. } => StatusCode::BAD_REQUEST,
            ApiError::ComputationOverflow => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::IndicatorComputeFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::DataSourceInterrupted => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::DataSourceRateLimited => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::AiServiceTimeout => StatusCode::GATEWAY_TIMEOUT,
            ApiError::AiServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// 回傳對應的 error_code 字串，對應 API_CONTRACT.md 的錯誤碼清單
    pub fn error_code(&self) -> &str {
        match self {
            ApiError::SymbolNotFound { .. } => "SYMBOL_NOT_FOUND",
            ApiError::QueryRangeTooLarge { .. } => "QUERY_RANGE_TOO_LARGE",
            ApiError::InvalidIndicatorConfig { .. } => "INVALID_INDICATOR_CONFIG",
            ApiError::IndicatorComputeFailed { .. } => "INDICATOR_COMPUTE_FAILED",
            ApiError::DataSourceInterrupted => "DATA_SOURCE_INTERRUPTED",
            ApiError::DataSourceRateLimited => "DATA_SOURCE_RATE_LIMITED",
            ApiError::ComputationOverflow => "COMPUTATION_OVERFLOW",
            ApiError::AiServiceTimeout => "AI_SERVICE_TIMEOUT",
            ApiError::AiServiceUnavailable => "AI_SERVICE_UNAVAILABLE",
            ApiError::Internal(_) => "INTERNAL_SERVER_ERROR",
        }
    }

    /// 是否有 fallback 可用（降級場景）
    pub fn fallback_available(&self) -> bool {
        matches!(
            self,
            ApiError::AiServiceTimeout | ApiError::AiServiceUnavailable
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_code = self.error_code().to_string();
        let message = self.to_string();
        let fallback = self.fallback_available();

        // Internal error 不對外暴露原始訊息
        let message = if matches!(self, ApiError::Internal(_)) {
            tracing::error!(error = %self, "Unexpected internal error");
            "An unexpected error occurred. Please try again later.".to_string()
        } else {
            message
        };

        let body = ErrorResponse {
            error_code,
            message,
            fallback_available: fallback,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            request_id: None,
        };

        (status, Json(body)).into_response()
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_not_found_status_code() {
        let err = ApiError::SymbolNotFound {
            symbol: "9999".to_string(),
        };
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(err.error_code(), "SYMBOL_NOT_FOUND");
    }

    #[test]
    fn test_query_range_too_large_status_code() {
        let err = ApiError::QueryRangeTooLarge {
            requested: 3000,
            max: 2000,
        };
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(err.error_code(), "QUERY_RANGE_TOO_LARGE");
    }

    #[test]
    fn test_ai_timeout_has_fallback() {
        let err = ApiError::AiServiceTimeout;
        assert!(err.fallback_available());
    }

    #[test]
    fn test_symbol_not_found_no_fallback() {
        let err = ApiError::SymbolNotFound {
            symbol: "9999".to_string(),
        };
        assert!(!err.fallback_available());
    }

    #[test]
    fn test_computation_overflow_status_422() {
        let err = ApiError::ComputationOverflow;
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
