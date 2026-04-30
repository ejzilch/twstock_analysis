use crate::api::models::{ReliabilityLevel, SignalSource};
use crate::models::SignalType;
use serde::Serialize;

/// GET /api/v1/signals/{symbol} 的完整 response
#[derive(Debug, Clone, Serialize)]
pub struct SignalsApiResponse {
    pub symbol: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub signals: Vec<TradeSignalResponse>,
    pub count: usize,
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
}
