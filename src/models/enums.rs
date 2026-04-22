use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::fmt;
use std::str::FromStr;
use strum::{AsRefStr, Display, EnumString};

/// 外部資料來源
///
/// fetch.rs 內部做 normalization，對外統一輸出 RawCandle，
/// 上層模組不需感知來源差異。
/// serde rename 確保序列化結果為 "finmind" / "yfinance"，
/// 對應 API_CONTRACT.md 的 data_source 欄位格式。
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Display, AsRefStr, EnumString,
)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    /// 主力來源：台股 (TWSE / TPEX)，走排程限流
    #[strum(serialize = "finmind")]
    FinMind,
    /// 備用來源：補歷史資料用，禁止放在即時路徑
    #[strum(serialize = "yfinance")]
    YFinance,
}

/// 交易所
///
/// 對應 API_CONTRACT.md 的 exchange 欄位與 init_schema.sql 的 exchange 欄位。
/// serde UPPERCASE 確保序列化結果為 "TWSE" / "TPEX"。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, Type, AsRefStr, EnumString,
)]
#[sqlx(type_name = "text", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum Exchange {
    /// 台灣證券交易所
    #[strum(serialize = "TWSE")]
    Twse,
    /// 證券櫃檯買賣中心
    #[strum(serialize = "TPEX")]
    Tpex,
}

/// K 線時間週期
///
/// Rust enum 名稱不可以數字開頭，以具名 variant 搭配 serde rename
/// 確保序列化結果與 API_CONTRACT.md 的 interval 欄位格式一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interval {
    #[serde(rename = "1m")]
    OneMin,
    #[serde(rename = "5m")]
    FiveMin,
    #[serde(rename = "15m")]
    FifteenMin,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "4h")]
    FourHours,
    #[serde(rename = "1d")]
    OneDay,
}

impl Interval {
    /// 回傳對應的字串，用於 DB 查詢與 tracing log
    pub fn as_str(&self) -> &'static str {
        match self {
            Interval::OneMin => "1m",
            Interval::FiveMin => "5m",
            Interval::FifteenMin => "15m",
            Interval::OneHour => "1h",
            Interval::FourHours => "4h",
            Interval::OneDay => "1d",
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Interval {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1m" => Ok(Interval::OneMin),
            "5m" => Ok(Interval::FiveMin),
            "15m" => Ok(Interval::FifteenMin),
            "1h" => Ok(Interval::OneHour),
            "4h" => Ok(Interval::FourHours),
            "1d" => Ok(Interval::OneDay),
            other => {
                anyhow::bail!("Unknown interval: {other}. Valid values: 1m, 5m, 15m, 1h, 4h, 1d")
            }
        }
    }
}

/// 交易信號類型
///
/// 對應 API_CONTRACT.md 的 signal_type 欄位。
/// serde UPPERCASE 確保序列化結果為 "BUY" / "SELL"。
/// 規範只允許 BUY / SELL，不包含 HOLD。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SignalType {
    Buy,
    Sell,
}

impl fmt::Display for SignalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalType::Buy => write!(f, "BUY"),
            SignalType::Sell => write!(f, "SELL"),
        }
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_as_str_roundtrip() {
        let intervals = [
            Interval::OneMin,
            Interval::FiveMin,
            Interval::FifteenMin,
            Interval::OneHour,
            Interval::FourHours,
            Interval::OneDay,
        ];
        for interval in intervals {
            let parsed: Interval = interval.as_str().parse().unwrap();
            assert_eq!(parsed, interval);
        }
    }

    #[test]
    fn test_interval_display_matches_as_str() {
        assert_eq!(Interval::OneHour.to_string(), "1h");
        assert_eq!(Interval::OneDay.to_string(), "1d");
    }

    #[test]
    fn test_interval_from_str_invalid_returns_error() {
        assert!("2h".parse::<Interval>().is_err());
        assert!("".parse::<Interval>().is_err());
    }

    #[test]
    fn test_interval_serde_roundtrip() {
        let json = serde_json::to_string(&Interval::OneHour).unwrap();
        let parsed: Interval = serde_json::from_str(&json).unwrap();
        assert_eq!(json, "\"1h\"");
        assert_eq!(parsed, Interval::OneHour);
    }

    #[test]
    fn test_exchange_display() {
        assert_eq!(Exchange::Twse.to_string(), "TWSE");
        assert_eq!(Exchange::Tpex.to_string(), "TPEX");
    }

    #[test]
    fn test_exchange_from_str_case_insensitive() {
        assert_eq!("twse", Exchange::Twse.to_string());
        assert_eq!("TPEX", Exchange::Tpex.to_string());
    }

    #[test]
    fn test_signal_type_no_hold_variant() {
        // 確認 HOLD 不存在，避免日後誤加
        let buy = SignalType::Buy;
        let sell = SignalType::Sell;
        assert_eq!(buy.to_string(), "BUY");
        assert_eq!(sell.to_string(), "SELL");
    }

    #[test]
    fn test_signal_type_serde() {
        let json = serde_json::to_string(&SignalType::Buy).unwrap();
        assert_eq!(json, "\"BUY\"");
    }
}
