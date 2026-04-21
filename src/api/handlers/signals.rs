use crate::ai_client::client::{PredictRequest, PredictResponse};
use crate::api::middleware::ApiError;
use crate::api::models::enums::{ReliabilityLevel, SignalSource};
use crate::api::models::request::SignalsQueryParams;
use crate::api::models::response::{SignalsApiResponse, TradeSignalResponse};
use crate::app_state::AppState;
use crate::constants::{ERROR_AI_SERVICE_TIMEOUT, ERROR_AI_SERVICE_UNAVAILABLE};
use crate::core::BridgeError;
use crate::models::enums::SignalType;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::collections::HashMap;
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

/// GET /api/v1/signals/{symbol}
///
/// AI 服務正常時回傳 source=ai_ensemble。
/// AI 超時或不可用時自動降級，回傳 source=technical_only。
/// fallback_reason 標注降級原因。
pub async fn signals_handler(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(params): Query<SignalsQueryParams>,
) -> Result<Json<SignalsApiResponse>, ApiError> {
    // 取得指標資料（快取優先）
    let indicators: HashMap<String, f64> = fetch_indicators_for_signal(&state, &symbol).await?;

    // 嘗試 AI 預測，10s timeout 後自動降級
    let predict_request = PredictRequest {
        request_id: Uuid::new_v4().to_string(),
        symbol: symbol.clone(),
        indicators: indicators.clone(),
        lookback_hours: crate::constants::AI_SERVICE_TIMEOUT_SECS as i64,
    };

    let signal = match timeout(
        Duration::from_secs(crate::constants::AI_SERVICE_TIMEOUT_SECS),
        state.ai_client.predict(&predict_request),
    )
    .await
    {
        // AI 正常回應
        Ok(Ok(prediction)) => build_ai_signal(&symbol, &prediction, params.from_ms),

        // AI 回傳錯誤
        Ok(Err(bridge_error)) => {
            let fallback_reason = match &bridge_error {
                BridgeError::PythonTimeout { .. } => ERROR_AI_SERVICE_TIMEOUT,
                BridgeError::PythonConnectionLost { .. } => ERROR_AI_SERVICE_UNAVAILABLE,
                BridgeError::PythonInternalError { message, traceback } => {
                    tracing::error!(
                        symbol = %symbol,
                        python_error = %message,
                        python_traceback = traceback.as_deref().unwrap_or("none"),
                        "Python internal error, falling back to technical signal"
                    );
                    ERROR_AI_SERVICE_UNAVAILABLE
                }
                _ => ERROR_AI_SERVICE_UNAVAILABLE,
            };

            tracing::warn!(
                symbol = %symbol,
                bridge_error = %bridge_error,
                fallback_reason = fallback_reason,
                "AI service error, falling back to technical signal"
            );

            build_technical_fallback_signal(&symbol, &indicators, fallback_reason, params.from_ms)
        }

        // Timeout
        Err(_elapsed) => {
            tracing::warn!(
                symbol = %symbol,
                timeout_secs = crate::constants::AI_SERVICE_TIMEOUT_SECS,
                fallback_reason = ERROR_AI_SERVICE_TIMEOUT,
                "AI service timed out, falling back to technical signal"
            );
            build_technical_fallback_signal(
                &symbol,
                &indicators,
                ERROR_AI_SERVICE_TIMEOUT,
                params.from_ms,
            )
        }
    };

    Ok(Json(SignalsApiResponse {
        symbol: symbol.clone(),
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        count: 1,
        signals: vec![signal],
    }))
}

// ── 信號建構函數 ──────────────────────────────────────────────────────────────

fn build_ai_signal(
    _symbol: &str,
    prediction: &PredictResponse,
    timestamp_ms: i64,
) -> TradeSignalResponse {
    let signal_type = if prediction.up_probability > prediction.down_probability {
        SignalType::Buy
    } else {
        SignalType::Sell
    };

    let reliability = ReliabilityLevel::from_confidence(prediction.confidence_score);

    let reason = format!(
        "AI ensemble prediction: up={:.2}, down={:.2}, confidence={:.2}, model={}",
        prediction.up_probability,
        prediction.down_probability,
        prediction.confidence_score,
        prediction.model_version,
    );

    TradeSignalResponse {
        id: format!("sig-{}", Uuid::new_v4()),
        timestamp_ms,
        signal_type: signal_type,
        confidence: prediction.confidence_score,
        entry_price: 0.0, // 由策略層決定，目前佔位
        target_price: 0.0,
        stop_loss: 0.0,
        reason,
        source: SignalSource::AiEnsemble,
        reliability,
        fallback_reason: None,
    }
}

fn build_technical_fallback_signal(
    _symbol: &str,
    indicators: &std::collections::HashMap<String, f64>,
    fallback_reason: &str,
    timestamp_ms: i64,
) -> TradeSignalResponse {
    // 簡單的技術指標規則：RSI < 30 買入，RSI > 70 賣出
    let rsi = indicators.get("rsi").copied().unwrap_or(50.0);
    let (signal_type, confidence) = if rsi < 30.0 {
        (SignalType::Buy, 0.4)
    } else if rsi > 70.0 {
        (SignalType::Sell, 0.4)
    } else {
        (SignalType::Buy, 0.3) // 無明確信號時給低信心度 BUY
    };

    TradeSignalResponse {
        id: format!("sig-{}", Uuid::new_v4()),
        timestamp_ms,
        signal_type: signal_type,
        confidence,
        entry_price: 0.0,
        target_price: 0.0,
        stop_loss: 0.0,
        reason: format!("Technical indicator fallback: RSI={rsi:.1}"),
        source: SignalSource::TechnicalOnly,
        reliability: ReliabilityLevel::Low,
        fallback_reason: Some(fallback_reason.to_string()),
    }
}

// ── 私有查詢函數 ──────────────────────────────────────────────────────────────

async fn fetch_indicators_for_signal(
    state: &AppState,
    symbol: &str,
) -> Result<std::collections::HashMap<String, f64>, ApiError> {
    // 從 Redis 快取取最新指標，若無則回傳空 map（降級處理）
    let cache_key = format!("indicators:{symbol}:1h");

    let mut conn = state.redis_client.clone();
    let cached: redis::RedisResult<Option<String>> = redis::cmd("GET")
        .arg(&cache_key)
        .query_async(&mut conn)
        .await;

    if let Ok(Some(json_str)) = cached {
        if let Ok(indicators) =
            serde_json::from_str::<std::collections::HashMap<String, f64>>(&json_str)
        {
            return Ok(indicators);
        }
    }

    tracing::warn!(
        symbol = %symbol,
        "No cached indicators found for signal generation, using empty indicators"
    );
    Ok(std::collections::HashMap::new())
}
