/// BridgeError 統一錯誤列舉。
/// 在現有定義基礎上新增 DatabaseError variant，
/// 供 sync_log_create / fetch_final_sync_state 等 DB 操作使用。
pub mod indicators;
pub mod strategy;

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
        raw_response: String,
    },

    /// Python 端明確回傳錯誤訊息與堆疊
    #[error("Python reported internal error: {message}")]
    PythonInternalError {
        message: String,
        traceback: Option<String>,
    },

    /// FinMind / 外部資料來源錯誤
    #[error("FinMind data source error: {context}")]
    FinMindDataSourceError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // ── 新增 variant（手動同步所需）─────────────────────────────────────────
    /// PostgreSQL 操作失敗（sqlx 錯誤包裝）
    /// 用於 sync_log_create / sync_log_update_* / fetch_final_sync_state
    #[error("Database error: {context}")]
    DatabaseError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Redis / 快取操作失敗
    #[error("Cache error: {context}")]
    CacheError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 內部邏輯錯誤（序列化 / 反序列化失敗等）
    #[error("Internal error: {context}")]
    InternalError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

// ── 方便建構的 impl ───────────────────────────────────────────────────────────

impl BridgeError {
    /// 從 sqlx::Error 建立 DatabaseError。
    pub fn from_db(
        context: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::DatabaseError {
            context: context.into(),
            source: Some(Box::new(e)),
        }
    }

    /// 從字串訊息建立 DatabaseError（不帶 source）。
    pub fn db(context: impl Into<String>) -> Self {
        Self::DatabaseError {
            context: context.into(),
            source: None,
        }
    }

    /// 從 redis::RedisError 建立 CacheError。
    pub fn from_cache(
        context: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::CacheError {
            context: context.into(),
            source: Some(Box::new(e)),
        }
    }

    /// 從字串訊息建立 InternalError。
    pub fn internal(context: impl Into<String>) -> Self {
        Self::InternalError {
            context: context.into(),
            source: None,
        }
    }
}
