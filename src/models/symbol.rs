use crate::data::models::DataSource;
use crate::models::Exchange;
use serde::{Deserialize, Serialize};

/// 系統動態管理的股票元數據
///
/// 由每日 02:00 排程從 FinMind 同步寫入，對應 DB 的 symbols 資料表。
/// 透過 GET /api/v1/symbols 對外回傳，前端股票選擇器從此載入。
///
/// 下市標的標記 is_active = false，保留歷史資料不刪除。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMeta {
    pub symbol: String,
    pub name: String,
    pub exchange: Exchange, // "TWSE" / "TPEX"
    pub data_source: DataSource,
    pub earliest_available_ms: i64,
    pub latest_available_ms: i64,
    pub is_active: bool,
    pub updated_at_ms: i64, // 最後一次從 FinMind 同步的時間
}
