/// Service 共用 types
///
/// 只用於存放 Service 共用 types
use crate::models::enums::SymbolSyncStatus;
use serde::{Deserialize, Serialize};

/// 單一股票的同步進度
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct SymbolProgress {
    pub symbol: String,
    pub name: String,
    pub status: SymbolSyncStatus,
    /// 缺口 A（歷史段）進度，None 表示無此缺口
    pub gap_a: Option<GapProgress>,
    /// 缺口 B（近期段）進度，None 表示無此缺口
    pub gap_b: Option<GapProgress>,
}

/// 單一缺口的進度
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct GapProgress {
    pub from_ms: i64,
    pub to_ms: i64,
    pub inserted: i32,
    pub skipped: i32,
    pub failed: i32,
    pub completed: bool,
}
