use crate::ai_client::client::PredictRequest;
use crate::api::handlers::health::AppState;
use crate::api::middleware::ApiError;
use axum::{extract::State, Json};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::timeout;

// ── POST /api/v1/predict ──────────────────────────────────────────────────────

/// POST /api/v1/predict 的請求結構
#[derive(Debug, Deserialize)]
pub struct PredictApiRequest {
    pub request_id: String,
    pub symbol: String,
    pub indicators: HashMap<String, f64>,
    pub lookback_hours: i64,
}

/// POST /api/v1/predict
///
/// Rust Gateway 轉發至 Python AI Service。
/// AI 超時或不可用時回傳 503 / 504，不自動降級（降級邏輯在 signals handler）。
pub async fn predict_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PredictApiRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let predict_request = PredictRequest {
        request_id: request.request_id,
        symbol: request.symbol.clone(),
        indicators: request.indicators,
        lookback_hours: request.lookback_hours,
    };

    let result = timeout(
        Duration::from_secs(crate::constants::TIMEOUT_AI_SERVICE_SECS),
        state.ai_client.predict(&predict_request),
    )
    .await;

    match result {
        Ok(Ok(prediction)) => Ok(Json(serde_json::json!({
            "symbol":           prediction.symbol,
            "up_probability":   prediction.up_probability,
            "down_probability": prediction.down_probability,
            "confidence_score": prediction.confidence_score,
            "model_version":    prediction.model_version,
            "inference_time_ms": prediction.inference_time_ms,
            "computed_at_ms":   prediction.computed_at_ms,
        }))),

        Ok(Err(bridge_error)) => {
            tracing::error!(
                symbol = %request.symbol,
                error  = %bridge_error,
                "AI service error in predict handler"
            );
            Err(ApiError::AiServiceUnavailable)
        }

        Err(_timeout) => {
            tracing::warn!(
                symbol       = %request.symbol,
                timeout_secs = crate::constants::TIMEOUT_AI_SERVICE_SECS,
                "AI service timed out in predict handler"
            );
            Err(ApiError::AiServiceTimeout)
        }
    }
}
