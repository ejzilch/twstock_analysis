use anyhow::Context;

/// 序列化格式
///
/// 依 candle_count 自動選擇，對應 ARCH_DESIGN.md 序列化格式策略：
/// - candle_count <= 1000: JSON（維護性優先）
/// - candle_count >  1000: MsgPack（效能優先）
#[derive(Debug, Clone, PartialEq)]
pub enum SerializationFormat {
    Json,
    MsgPack,
}

impl SerializationFormat {
    /// 依 K 線數量自動選擇序列化格式
    pub fn select_by_candle_count(candle_count: usize) -> Self {
        if candle_count > 1000 {
            SerializationFormat::MsgPack
        } else {
            SerializationFormat::Json
        }
    }

    /// 回傳對應的 Content-Type header 值
    pub fn content_type(&self) -> &'static str {
        match self {
            SerializationFormat::Json => "application/json",
            SerializationFormat::MsgPack => "application/msgpack",
        }
    }
}

/// 將資料序列化為指定格式的 bytes
pub fn serialize<T: serde::Serialize>(
    value: &T,
    format: &SerializationFormat,
) -> anyhow::Result<Vec<u8>> {
    match format {
        SerializationFormat::Json => serde_json::to_vec(value).context("JSON serialization failed"),
        SerializationFormat::MsgPack => {
            rmp_serde::to_vec_named(value).context("MsgPack serialization failed")
        }
    }
}

/// 依 Content-Type header 自動反序列化 bytes
pub fn deserialize<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    content_type: &str,
) -> anyhow::Result<T> {
    if content_type.contains("application/msgpack") {
        rmp_serde::from_slice(bytes).context("MsgPack deserialization failed")
    } else {
        serde_json::from_slice(bytes).context("JSON deserialization failed")
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestPayload {
        symbol: String,
        value: f64,
    }

    #[test]
    fn test_format_selection_below_threshold() {
        assert_eq!(
            SerializationFormat::select_by_candle_count(1000),
            SerializationFormat::Json
        );
    }

    #[test]
    fn test_format_selection_above_threshold() {
        assert_eq!(
            SerializationFormat::select_by_candle_count(1001),
            SerializationFormat::MsgPack
        );
    }

    #[test]
    fn test_json_roundtrip() {
        let payload = TestPayload {
            symbol: "2330".to_string(),
            value: 150.5,
        };
        let bytes = serialize(&payload, &SerializationFormat::Json).unwrap();
        let result: TestPayload = deserialize(&bytes, "application/json").unwrap();
        assert_eq!(payload, result);
    }

    #[test]
    fn test_msgpack_roundtrip() {
        let payload = TestPayload {
            symbol: "2330".to_string(),
            value: 150.5,
        };
        let bytes = serialize(&payload, &SerializationFormat::MsgPack).unwrap();
        let result: TestPayload = deserialize(&bytes, "application/msgpack").unwrap();
        assert_eq!(payload, result);
    }

    #[test]
    fn test_content_type_json() {
        assert_eq!(SerializationFormat::Json.content_type(), "application/json");
    }

    #[test]
    fn test_content_type_msgpack() {
        assert_eq!(
            SerializationFormat::MsgPack.content_type(),
            "application/msgpack"
        );
    }
}
