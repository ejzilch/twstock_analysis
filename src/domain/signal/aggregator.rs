/// 信號聚合器（純 domain 邏輯，零 I/O）
///
/// 職責：
///   - 將 AI 預測結果轉換為 TradeSignalResponse
///   - 將技術指標轉換為 fallback TradeSignalResponse
///   - 計算信號可靠性等級
///
/// 不依賴 AppState、HTTP、DB、Redis。
/// 所有副作用（AI 呼叫、Redis 查詢）由 SignalService 負責。
use crate::ai_client::client::PredictResponse;
use crate::models::enums::{ReliabilityLevel, SignalSource, SignalType};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// aggregator 的 model
#[derive(Debug, Clone, Serialize)]
pub struct TradeSignal {
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

// ── AI 信號建構 ───────────────────────────────────────────────────────────────

/// 從 AI 預測結果建構交易信號。
pub fn build_ai_signal(prediction: &PredictResponse, timestamp_ms: i64) -> TradeSignal {
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

    TradeSignal {
        id: format!("sig-{}", Uuid::new_v4()),
        timestamp_ms,
        signal_type,
        confidence: prediction.confidence_score,
        entry_price: 0.0,
        target_price: 0.0,
        stop_loss: 0.0,
        reason,
        source: SignalSource::AiEnsemble,
        reliability,
        fallback_reason: None,
    }
}

// ── Fallback 信號建構 ─────────────────────────────────────────────────────────

/// 從技術指標建構 fallback 交易信號（AI 不可用時使用）。
///
/// 規則：RSI < 30 → Buy，RSI > 70 → Sell，否則低信心 Buy。
/// fallback_reason 標注觸發降級的原因（如 "AI_SERVICE_TIMEOUT"）。
pub fn build_technical_fallback_signal(
    indicators: &HashMap<String, f64>,
    fallback_reason: &str,
    timestamp_ms: i64,
) -> TradeSignal {
    let rsi = indicators.get("rsi").copied().unwrap_or(50.0);

    let (signal_type, confidence) = if rsi < 30.0 {
        (SignalType::Buy, 0.4)
    } else if rsi > 70.0 {
        (SignalType::Sell, 0.4)
    } else {
        (SignalType::Buy, 0.3)
    };

    TradeSignal {
        id: format!("sig-{}", Uuid::new_v4()),
        timestamp_ms,
        signal_type,
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

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_prediction(up: f64, down: f64, confidence: f64) -> PredictResponse {
        PredictResponse {
            symbol: "2330".to_string(),
            up_probability: up,
            down_probability: down,
            confidence_score: confidence,
            model_version: "v1.0".to_string(),
            inference_time_ms: 10,
            computed_at_ms: 0,
        }
    }

    #[test]
    fn test_build_ai_signal_buy_when_up_higher() {
        let pred = make_prediction(0.7, 0.3, 0.8);
        let signal = build_ai_signal(&pred, 0);
        assert_eq!(signal.signal_type, SignalType::Buy);
        assert_eq!(signal.source, SignalSource::AiEnsemble);
        assert!(signal.fallback_reason.is_none());
    }

    #[test]
    fn test_build_ai_signal_sell_when_down_higher() {
        let pred = make_prediction(0.3, 0.7, 0.8);
        let signal = build_ai_signal(&pred, 0);
        assert_eq!(signal.signal_type, SignalType::Sell);
    }

    #[test]
    fn test_build_ai_signal_reliability_high() {
        let pred = make_prediction(0.8, 0.2, 0.75);
        let signal = build_ai_signal(&pred, 0);
        assert_eq!(signal.reliability, ReliabilityLevel::High);
    }

    #[test]
    fn test_build_ai_signal_reliability_medium() {
        let pred = make_prediction(0.8, 0.2, 0.6);
        let signal = build_ai_signal(&pred, 0);
        assert_eq!(signal.reliability, ReliabilityLevel::Medium);
    }

    #[test]
    fn test_build_ai_signal_reliability_low() {
        let pred = make_prediction(0.8, 0.2, 0.3);
        let signal = build_ai_signal(&pred, 0);
        assert_eq!(signal.reliability, ReliabilityLevel::Low);
    }

    #[test]
    fn test_fallback_buy_when_rsi_oversold() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 25.0);
        let signal = build_technical_fallback_signal(&indicators, "AI_SERVICE_TIMEOUT", 0);
        assert_eq!(signal.signal_type, SignalType::Buy);
        assert_eq!(signal.source, SignalSource::TechnicalOnly);
        assert_eq!(
            signal.fallback_reason.as_deref(),
            Some("AI_SERVICE_TIMEOUT")
        );
        assert!((signal.confidence - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fallback_sell_when_rsi_overbought() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 75.0);
        let signal = build_technical_fallback_signal(&indicators, "AI_SERVICE_UNAVAILABLE", 0);
        assert_eq!(signal.signal_type, SignalType::Sell);
    }

    #[test]
    fn test_fallback_low_confidence_when_rsi_neutral() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0);
        let signal = build_technical_fallback_signal(&indicators, "AI_SERVICE_TIMEOUT", 0);
        assert!((signal.confidence - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fallback_defaults_rsi_50_when_missing() {
        // RSI 缺失時預設 50，應走低信心 Buy
        let signal = build_technical_fallback_signal(&HashMap::new(), "AI_SERVICE_TIMEOUT", 0);
        assert_eq!(signal.signal_type, SignalType::Buy);
        assert!((signal.confidence - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_signal_id_is_unique() {
        let pred = make_prediction(0.7, 0.3, 0.8);
        let s1 = build_ai_signal(&pred, 0);
        let s2 = build_ai_signal(&pred, 0);
        assert_ne!(s1.id, s2.id);
    }
}
