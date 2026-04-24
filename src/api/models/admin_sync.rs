use crate::api::models::SyncMode;
use crate::models::enums::Interval;

use serde::{Deserialize, Serialize};

// ── Request ───────────────────────────────────────────────────────────────────

/// POST /api/v1/admin/sync 請求體
#[derive(Debug, Deserialize)]
pub struct ManualSyncRequest {
    /// 冪等性識別碼，由前端產生
    pub request_id: String,

    /// 冪等性識別碼，由前端產生
    pub mode: Option<SyncMode>,

    /// 要同步的股票代號清單，至少 1 檔
    pub symbols: Option<Vec<String>>,

    /// 是否全量回補（true: 忽略 from/to）
    pub full_sync: Option<bool>,
    /// 自訂起始日期（YYYY-MM-DD）
    pub from_date: Option<String>,
    /// 自訂結束日期（YYYY-MM-DD）
    pub to_date: Option<String>,
    /// 只同步指定 K 線刻度；空值代表全部
    pub intervals: Option<Vec<Interval>>,
}

// ── Response ──────────────────────────────────────────────────────────────────

/// POST /api/v1/admin/sync 成功回應（202）
#[derive(Debug, Serialize)]
pub struct ManualSyncAcceptedResponse {
    pub sync_id: String,
    pub status: SyncStatus,
    pub symbols: Vec<String>,
    pub estimated_requests: u64,
    pub estimated_hours: u64,
    pub started_at_ms: i64,
}

/// GET /api/v1/admin/sync/status 回應
#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    pub sync_id: String,
    pub status: SyncStatus,
    pub started_at_ms: i64,
    pub rate_limit: RateLimitInfo,
    pub progress: Vec<SymbolProgress>,
    pub summary: SyncSummary,
}

/// 同步任務狀態
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Running,
    RateLimitWaiting,
    Completed,
    Failed,
}

impl SyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatus::Running => "running",
            SyncStatus::RateLimitWaiting => "rate_limit_waiting",
            SyncStatus::Completed => "completed",
            SyncStatus::Failed => "failed",
        }
    }

    /// 是否仍在進行中（供前端判斷是否繼續輪詢）
    pub fn is_in_progress(&self) -> bool {
        matches!(self, SyncStatus::Running | SyncStatus::RateLimitWaiting)
    }
}

/// Rate limit 即時狀態
#[derive(Debug, Serialize)]
pub struct RateLimitInfo {
    pub used_this_hour: u32,
    pub limit_per_hour: u32,
    pub is_waiting: bool,
    /// 若 is_waiting == true，rate limit 解除的毫秒級 timestamp
    pub resume_at_ms: Option<i64>,
}

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

/// 單一股票的同步狀態
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolSyncStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
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

/// 整體同步結果摘要
#[derive(Debug, Serialize, Default, Clone, Deserialize)]
pub struct SyncSummary {
    pub total_symbols: usize,
    pub completed_symbols: usize,
    pub total_inserted: i32,
    pub total_skipped: i32,
    pub total_failed: i32,
}
