/// POST /api/v1/predict — 薄 handler
///
/// 直接呼叫 AI 服務，不做降級（降級邏輯在 signals handler / SignalService）。
/// AI 超時或不可用時回傳 503 / 504，由呼叫方決定如何處理。
use crate::ai_client::client::PredictRequest;
use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::constants::AI_SERVICE_TIMEOUT_SECS;
use crate::domain::BridgeError;
use axum::{extract::State, Json};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::timeout;

#[derive(Debug, Deserialize)]
pub struct PredictApiRequest {
    pub request_id: String,
    pub symbol: String,
    pub indicators: HashMap<String, f64>,
    pub lookback_hours: i64,
}

/// POST /api/v1/predict
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
        Duration::from_secs(AI_SERVICE_TIMEOUT_SECS),
        state.ai_client.predict(&predict_request),
    )
    .await;

    match result {
        Ok(Ok(p)) => Ok(Json(serde_json::json!({
            "symbol":           p.symbol,
            "up_probability":   p.up_probability,
            "down_probability": p.down_probability,
            "confidence_score": p.confidence_score,
            "model_version":    p.model_version,
            "inference_time_ms":p.inference_time_ms,
            "computed_at_ms":   p.computed_at_ms,
        }))),
        Ok(Err(BridgeError::PythonTimeout { .. })) | Err(_) => {
            tracing::warn!(symbol = %request.symbol, "AI service timed out in predict handler");
            Err(ApiError::AiServiceTimeout)
        }
        Ok(Err(e)) => {
            tracing::error!(symbol = %request.symbol, error = %e, "AI service error in predict handler");
            Err(ApiError::AiServiceUnavailable)
        }
    }
}
