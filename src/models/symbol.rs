use crate::data::models::DataSource;
use serde::{Deserialize, Serialize};

// 系統支援的股票元數據結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMeta {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    pub data_source: DataSource,
    pub earliest_available_ms: i64,
    pub latest_available_ms: i64,
    pub is_active: bool,
}
