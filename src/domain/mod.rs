/// 核心計算模組（Domain layer）
///
/// backtest:   純計算回測引擎 + 財務指標（零 I/O）
/// indicators: 技術指標計算（MA / RSI / MACD / Bollinger）
/// signals:     信號聚合（AI signal / technical fallback，零 I/O）
/// strategy:   交易信號策略
pub mod backtest;
pub mod indicators;
pub mod signal;
pub mod strategy;

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Python service returned error: status={status_code}, body={response_body}")]
    PythonServiceError {
        status_code: u16,
        response_body: String,
    },

    #[error("Python service connection lost: {reason}")]
    PythonConnectionLost { reason: String },

    #[error("Python service timed out after {timeout_secs}s for symbol={symbol}")]
    PythonTimeout { timeout_secs: u64, symbol: String },

    #[error("Python response deserialization failed: {detail}")]
    PythonResponseMalformed {
        detail: String,
        raw_response: String,
    },

    #[error("Python reported internal error: {message}")]
    PythonInternalError {
        message: String,
        traceback: Option<String>,
    },

    #[error("FinMind data source error: {context}")]
    FinMindDataSourceError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Database error: {context}")]
    DatabaseError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Cache error: {context}")]
    CacheError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Internal error: {context}")]
    InternalError {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl BridgeError {
    pub fn from_db(
        context: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::DatabaseError {
            context: context.into(),
            source: Some(Box::new(e)),
        }
    }

    pub fn from_cache(
        context: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::CacheError {
            context: context.into(),
            source: Some(Box::new(e)),
        }
    }

    pub fn internal(context: impl Into<String>) -> Self {
        Self::InternalError {
            context: context.into(),
            source: None,
        }
    }
}
