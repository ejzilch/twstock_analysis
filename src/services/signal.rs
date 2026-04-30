/// SignalService — 信號業務流程協調者（Service layer）
///
/// 職責：
///   1. 從 Redis 取得最新指標（快取優先）
///   2. 呼叫 AI 服務取得預測（含 timeout）
///   3. AI 失敗時自動降級為技術指標 fallback
///   4. 呼叫 aggregator 建構 TradeSignalResponse
///
/// handler 只需呼叫 `SignalService::generate()`，不含任何 AI 或降級邏輯。
use crate::ai_client::client::PredictRequest;
use crate::api::middleware::ApiError;
use crate::api::signal::dto::response::{SignalsApiResponse, TradeSignalResponse};
use crate::app_state::AppState;
use crate::constants::{
    AI_SERVICE_TIMEOUT_SECS, ERROR_AI_SERVICE_TIMEOUT, ERROR_AI_SERVICE_UNAVAILABLE,
};
use crate::domain::signal::aggregator::{build_ai_signal, build_technical_fallback_signal};
use crate::domain::BridgeError;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

pub struct SignalService;

impl SignalService {
    /// 產生交易信號（AI 優先，降級為技術指標）。
    pub async fn generate(
        state: &AppState,
        symbol: &str,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<SignalsApiResponse, ApiError> {
        // ── Step 1: 取得指標 ──────────────────────────────────────────────────
        let indicators = Self::fetch_indicators(state, symbol).await;

        // ── Step 2: 嘗試 AI 預測，timeout 後自動降級 ─────────────────────────
        let signal = Self::generate_signal(state, symbol, &indicators, from_ms).await;

        Ok(SignalsApiResponse {
            symbol: symbol.to_string(),
            from_ms,
            to_ms,
            count: 1,
            signals: vec![signal],
        })
    }

    // ── 私有：信號產生（含 fallback） ─────────────────────────────────────────

    async fn generate_signal(
        state: &AppState,
        symbol: &str,
        indicators: &HashMap<String, f64>,
        timestamp_ms: i64,
    ) -> TradeSignalResponse {
        let predict_request = PredictRequest {
            request_id: Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            indicators: indicators.clone(),
            lookback_hours: AI_SERVICE_TIMEOUT_SECS as i64,
        };

        match timeout(
            Duration::from_secs(AI_SERVICE_TIMEOUT_SECS),
            state.ai_client.predict(&predict_request),
        )
        .await
        {
            // AI 正常回應
            Ok(Ok(prediction)) => {
                tracing::debug!(symbol = %symbol, "AI signal generated");
                build_ai_signal(&prediction, timestamp_ms)
            }

            // AI 回傳錯誤 → 降級
            Ok(Err(bridge_error)) => {
                let fallback_reason = Self::bridge_error_to_reason(&bridge_error, symbol);
                tracing::warn!(
                    symbol = %symbol,
                    error = %bridge_error,
                    fallback_reason = %fallback_reason,
                    "AI error, falling back to technical signal"
                );
                build_technical_fallback_signal(indicators, fallback_reason, timestamp_ms)
            }

            // Timeout → 降級
            Err(_elapsed) => {
                tracing::warn!(
                    symbol = %symbol,
                    timeout_secs = AI_SERVICE_TIMEOUT_SECS,
                    "AI timeout, falling back to technical signal"
                );
                build_technical_fallback_signal(indicators, ERROR_AI_SERVICE_TIMEOUT, timestamp_ms)
            }
        }
    }

    /// BridgeError → fallback_reason 字串（並處理 Python internal error log）
    fn bridge_error_to_reason(error: &BridgeError, symbol: &str) -> &'static str {
        match error {
            BridgeError::PythonTimeout { .. } => ERROR_AI_SERVICE_TIMEOUT,
            BridgeError::PythonInternalError { message, traceback } => {
                tracing::error!(
                    symbol = %symbol,
                    python_error = %message,
                    python_traceback = traceback.as_deref().unwrap_or("none"),
                    "Python internal error, falling back"
                );
                ERROR_AI_SERVICE_UNAVAILABLE
            }
            _ => ERROR_AI_SERVICE_UNAVAILABLE,
        }
    }

    // ── 私有：指標查詢 ────────────────────────────────────────────────────────

    /// 從 Redis 取得最新指標，失敗時靜默回傳空 map（降級處理）。
    async fn fetch_indicators(state: &AppState, symbol: &str) -> HashMap<String, f64> {
        let cache_key = format!("indicators:{symbol}:1h");
        let mut conn = state.redis_client.clone();

        let cached: redis::RedisResult<Option<String>> = redis::cmd("GET")
            .arg(&cache_key)
            .query_async(&mut conn)
            .await;

        if let Ok(Some(json_str)) = cached {
            if let Ok(indicators) = serde_json::from_str::<HashMap<String, f64>>(&json_str) {
                return indicators;
            }
        }

        tracing::warn!(
            symbol = %symbol,
            "No cached indicators found, using empty map for signal generation"
        );
        HashMap::new()
    }
}
