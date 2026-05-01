use serde::{Deserialize, Serialize};

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

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_serde_lowercase() {
        let json = serde_json::to_string(&HealthStatus::Ok).unwrap();
        assert_eq!(json, "\"ok\"");

        let json = serde_json::to_string(&HealthStatus::Degraded).unwrap();
        assert_eq!(json, "\"degraded\"");
    }

    #[test]
    fn test_observability_status_display() {
        assert_eq!(ObservabilityStatus::Ok.to_string(), "ok");
        assert_eq!(ObservabilityStatus::Warning.to_string(), "warning");
        assert_eq!(ObservabilityStatus::Critical.to_string(), "critical");
    }
}
