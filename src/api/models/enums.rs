use serde::{Deserialize, Serialize};

/// 信號來源
///
/// 標示交易信號由何處產生，前端依此決定顯示方式。
/// serde snake_case 對應 API_CONTRACT.md 的 source 欄位格式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSource {
    /// Python AI 模型集成預測，正常狀態
    AiEnsemble,
    /// 技術指標 fallback（AI 不可用時）
    TechnicalOnly,
    /// 人工干預信號
    ManualOverride,
}

/// 信號可靠性等級
///
/// 標示 AI 預測或技術指標信號的可信度。
/// 前端依此決定 reliability badge 的顯示顏色（見 FRONTEND_SPEC.md）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReliabilityLevel {
    /// AI 正常，confidence > 0.7
    High,
    /// AI 正常，confidence 0.5 ~ 0.7
    Medium,
    /// AI 超時 fallback，或技術指標強度弱
    Low,
    /// 無法取得任何信號
    Unknown,
}

impl ReliabilityLevel {
    /// 依 AI confidence 值計算可靠性等級
    pub fn from_confidence(confidence: f64) -> Self {
        if confidence > 0.7 {
            ReliabilityLevel::High
        } else if confidence >= 0.5 {
            ReliabilityLevel::Medium
        } else {
            ReliabilityLevel::Low
        }
    }
}

/// 系統健康狀態
///
/// 用於 GET /api/v1/health 與 GET /health/integrity 的 status 欄位。
/// Copy 允許在多個 struct 欄位間共用而不需要 clone。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// 所有元件正常
    Ok,
    /// 部分元件降級，系統仍可運作
    Degraded,
    /// 嚴重錯誤，需立即介入
    Error,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Ok => write!(f, "ok"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Error => write!(f, "error"),
        }
    }
}

/// Observability 指標的告警等級
///
/// 用於 /health/integrity 的各指標 status 欄位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObservabilityStatus {
    Ok,
    Warning,
    Critical,
}

impl std::fmt::Display for ObservabilityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObservabilityStatus::Ok => write!(f, "ok"),
            ObservabilityStatus::Warning => write!(f, "warning"),
            ObservabilityStatus::Critical => write!(f, "critical"),
        }
    }
}

/// K 線資料來源
///
/// 用於 GET /api/v1/candles/{symbol} response 的 source 欄位，
/// 標示本次資料是從資料庫查詢還是從 Redis 快取取得。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FetchSource {
    Database,
    Cache,
}

impl std::fmt::Display for FetchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchSource::Database => write!(f, "database"),
            FetchSource::Cache => write!(f, "cache"),
        }
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reliability_from_confidence_boundaries() {
        assert_eq!(
            ReliabilityLevel::from_confidence(0.71),
            ReliabilityLevel::High
        );
        assert_eq!(
            ReliabilityLevel::from_confidence(0.70),
            ReliabilityLevel::Medium
        );
        assert_eq!(
            ReliabilityLevel::from_confidence(0.50),
            ReliabilityLevel::Medium
        );
        assert_eq!(
            ReliabilityLevel::from_confidence(0.49),
            ReliabilityLevel::Low
        );
    }

    #[test]
    fn test_health_status_serde_lowercase() {
        let json = serde_json::to_string(&HealthStatus::Ok).unwrap();
        assert_eq!(json, "\"ok\"");

        let json = serde_json::to_string(&HealthStatus::Degraded).unwrap();
        assert_eq!(json, "\"degraded\"");
    }

    #[test]
    fn test_signal_source_serde_snake_case() {
        let json = serde_json::to_string(&SignalSource::AiEnsemble).unwrap();
        assert_eq!(json, "\"ai_ensemble\"");

        let json = serde_json::to_string(&SignalSource::TechnicalOnly).unwrap();
        assert_eq!(json, "\"technical_only\"");
    }

    #[test]
    fn test_fetch_source_display() {
        assert_eq!(FetchSource::Database.to_string(), "database");
        assert_eq!(FetchSource::Cache.to_string(), "cache");
    }

    #[test]
    fn test_observability_status_display() {
        assert_eq!(ObservabilityStatus::Ok.to_string(), "ok");
        assert_eq!(ObservabilityStatus::Warning.to_string(), "warning");
        assert_eq!(ObservabilityStatus::Critical.to_string(), "critical");
    }
}
