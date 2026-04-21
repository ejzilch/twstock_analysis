use crate::ai_client::serialization::{deserialize, serialize, SerializationFormat};
use crate::core::BridgeError;
use anyhow::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Python AI Service timeout，對應 ARCH_DESIGN.md 定義
const AI_SERVICE_TIMEOUT_SECS: u64 = 10;

// ── 請求 / 回應結構 ───────────────────────────────────────────────────────────

/// 傳送給 Python /predict 端點的請求
#[derive(Debug, Serialize)]
pub struct PredictRequest {
    pub request_id: String,
    pub symbol: String,
    pub indicators: std::collections::HashMap<String, f64>,
    pub lookback_hours: i64,
}

/// Python /predict 端點的回應
#[derive(Debug, Deserialize)]
pub struct PredictResponse {
    pub symbol: String,
    pub up_probability: f64,
    pub down_probability: f64,
    pub confidence_score: f64,
    pub model_version: String,
    pub inference_time_ms: i64,
    pub computed_at_ms: i64,
}

/// Python 端的 error envelope 格式
///
/// 對應 ARCH_DESIGN.md 約定的 Python 非 2xx 回應格式。
#[derive(Debug, Deserialize)]
struct PythonErrorEnvelope {
    pub message: String,
    pub traceback: Option<String>,
}

// ── AI Client ─────────────────────────────────────────────────────────────────

/// Python AI Service HTTP 客戶端
///
/// 封裝與 Python FastAPI 的通訊，包含：
/// - BridgeError 轉換（各種失敗情境 -> 統一錯誤類型）
/// - JSON / MsgPack 序列化自動選擇
/// - Python traceback 寫入 tracing log，不對外暴露
#[derive(Clone)]
pub struct AiServiceClient {
    client: Client,
    base_url: String,
}

impl AiServiceClient {
    /// 建立新的 AiServiceClient
    ///
    /// base_url 從環境變數 PYTHON_AI_SERVICE_URL 取得。
    /// timeout 固定 10s，對應 ARCH_DESIGN.md 規範。
    pub fn new(base_url: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(AI_SERVICE_TIMEOUT_SECS))
            .build()
            .context("Failed to build AI service HTTP client")?;

        Ok(Self { client, base_url })
    }

    /// 呼叫 Python /predict 端點取得 AI 預測結果
    ///
    /// 依 indicators 數量自動選擇序列化格式。
    /// 所有失敗情境轉換為 BridgeError，由呼叫方決定降級策略。
    pub async fn predict(&self, request: &PredictRequest) -> Result<PredictResponse, BridgeError> {
        // 依指標數量選擇序列化格式
        let candle_count = request.indicators.len();
        let format = SerializationFormat::select_by_candle_count(candle_count);
        let symbol = request.symbol.clone();

        let body =
            serialize(request, &format).map_err(|e| BridgeError::PythonResponseMalformed {
                detail: format!("Request serialization failed: {e}"),
                raw_response: String::new(),
            })?;

        let response = self
            .client
            .post(format!("{}/predict", self.base_url))
            .header("Content-Type", format.content_type())
            .body(body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    BridgeError::PythonTimeout {
                        timeout_secs: AI_SERVICE_TIMEOUT_SECS,
                        symbol: symbol.clone(),
                    }
                } else if e.is_connect() {
                    BridgeError::PythonConnectionLost {
                        reason: e.to_string(),
                    }
                } else {
                    BridgeError::PythonConnectionLost {
                        reason: format!("Request failed: {e}"),
                    }
                }
            })?;

        let status = response.status();

        if !status.is_success() {
            let status_code = status.as_u16();
            let body_bytes = response.bytes().await.unwrap_or_default();

            // 嘗試解析 Python error envelope
            if let Ok(envelope) = serde_json::from_slice::<PythonErrorEnvelope>(&body_bytes) {
                tracing::error!(
                    symbol           = %symbol,
                    python_error     = %envelope.message,
                    python_traceback = envelope.traceback.as_deref().unwrap_or("none"),
                    "Python internal error captured"
                );
                return Err(BridgeError::PythonInternalError {
                    message: envelope.message,
                    traceback: envelope.traceback,
                });
            }

            // 無法解析 envelope，回傳原始 body
            let body_str = String::from_utf8_lossy(&body_bytes).to_string();
            tracing::error!(
                symbol      = %symbol,
                status_code = status_code,
                body        = %body_str,
                "Python service returned non-2xx without error envelope"
            );
            return Err(BridgeError::PythonServiceError {
                status_code,
                response_body: body_str,
            });
        }

        // 解析成功回應
        let response_bytes =
            response
                .bytes()
                .await
                .map_err(|e| BridgeError::PythonConnectionLost {
                    reason: format!("Failed to read response body: {e}"),
                })?;

        let content_type = "application/json"; // Python 端固定回傳 JSON
        deserialize::<PredictResponse>(&response_bytes, content_type).map_err(|e| {
            BridgeError::PythonResponseMalformed {
                detail: e.to_string(),
                raw_response: String::from_utf8_lossy(&response_bytes).to_string(),
            }
        })
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_client_new_succeeds() {
        let client = AiServiceClient::new("http://localhost:8001".to_string());
        assert!(client.is_ok());
    }

    #[test]
    fn test_predict_request_serializes_to_json() {
        let request = PredictRequest {
            request_id: "req-001".to_string(),
            symbol: "2330".to_string(),
            indicators: std::collections::HashMap::from([
                ("ma20".to_string(), 150.5),
                ("rsi".to_string(), 55.2),
            ]),
            lookback_hours: 24,
        };
        let bytes = serialize(&request, &SerializationFormat::Json);
        assert!(bytes.is_ok());
    }
}
