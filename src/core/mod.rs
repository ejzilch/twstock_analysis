pub mod indicators;
pub mod strategy;

/// Rust 與 Python AI Service 之間通訊錯誤的統一分類
///
/// 每個 variant 攜帶足夠的上下文供除錯，
/// 由 ai_client/client.rs 捕捉後寫入 tracing log，
/// 對外 response 只回傳降級行為（source = technical_only），
/// 禁止將 traceback 或內部錯誤細節暴露給前端。
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// Python 服務主動回傳非 2xx，攜帶 HTTP status 與 body
    #[error("Python service returned error: status={status_code}, body={response_body}")]
    PythonServiceError {
        status_code: u16,
        response_body: String,
    },

    /// Python 程序崩潰或 OOM，連線被強制關閉
    #[error("Python service connection lost: {reason}")]
    PythonConnectionLost { reason: String },

    /// 請求超過 timeout 未回應
    #[error("Python service timed out after {timeout_secs}s for symbol={symbol}")]
    PythonTimeout { timeout_secs: u64, symbol: String },

    /// Python 回傳格式不符合預期 schema
    #[error("Python response deserialization failed: {detail}")]
    PythonResponseMalformed {
        detail: String,
        raw_response: String, // 寫入 tracing log，不對外暴露
    },

    /// Python 端明確回傳錯誤訊息與堆疊（透過 error envelope 傳遞）
    #[error("Python reported internal error: {message}")]
    PythonInternalError {
        message: String,
        traceback: Option<String>, // 寫入 tracing log，禁止對外暴露
    },
}
