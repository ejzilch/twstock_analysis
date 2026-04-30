/// GET /api/v1/signals/{symbol} 的查詢參數
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SignalsQueryParams {
    /// 開始時間戳（毫秒）
    pub from_ms: i64,
    /// 結束時間戳（毫秒）
    pub to_ms: i64,
}
