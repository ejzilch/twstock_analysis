use crate::api::middleware::ApiError;
use crate::api::models::request::SymbolsQueryParams;
use crate::app_state::AppState;
use crate::models::Exchange;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;

/// GET /api/v1/symbols response
#[derive(Debug, Serialize)]
pub struct SymbolsResponse {
    pub symbols: Vec<SymbolItem>,
    pub count: usize,
    pub last_synced_ms: i64,
}

/// 單筆 symbol 資料，對應 API_CONTRACT.md 的 Symbol schema
#[derive(Debug, Serialize)]
pub struct SymbolItem {
    pub symbol: String,
    pub name: String,
    pub exchange: Exchange,
    pub data_source: String,
    pub earliest_available_ms: i64,
    pub latest_available_ms: i64,
    pub is_active: bool,
    pub updated_at_ms: i64,
}

/// GET /api/v1/symbols
///
/// 回傳系統動態管理的股票清單。
/// 資料由每日 02:00 排程從 FinMind 同步，不即時呼叫外部 API。
pub async fn symbols_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SymbolsQueryParams>,
) -> Result<Json<SymbolsResponse>, ApiError> {
    let is_active = params.is_active();

    // 組裝 SQL 查詢條件
    let symbols = fetch_symbols(&state, params.exchange, is_active).await?;

    let last_synced_ms = fetch_last_synced_ms(&state).await.unwrap_or(0);
    let count = symbols.len();

    Ok(Json(SymbolsResponse {
        symbols,
        count,
        last_synced_ms,
    }))
}

// ── 私有查詢函數 ──────────────────────────────────────────────────────────────

async fn fetch_symbols(
    state: &AppState,
    exchange: Option<Exchange>,
    is_active: bool,
) -> Result<Vec<SymbolItem>, ApiError> {
    let rows = match exchange {
        Some(ex) => {
            sqlx::query_as!(
                SymbolRow,
                r#"
                SELECT symbol, name, exchange as "exchange: Exchange", data_source,
                       COALESCE(earliest_ms, 0) AS "earliest_ms!",
                       COALESCE(latest_ms, 0)   AS "latest_ms!",
                       is_active, updated_at_ms
                FROM symbols
                WHERE is_active = $1 AND exchange = $2
                ORDER BY symbol ASC
                "#,
                is_active,
                ex.to_string()
            )
            .fetch_all(&state.db_pool)
            .await
        }
        None => {
            sqlx::query_as!(
                SymbolRow,
                r#"
                SELECT symbol, name, exchange as "exchange: Exchange", data_source,
                       COALESCE(earliest_ms, 0) AS "earliest_ms!",
                       COALESCE(latest_ms, 0)   AS "latest_ms!",
                       is_active, updated_at_ms
                FROM symbols
                WHERE is_active = $1
                ORDER BY symbol ASC
                "#,
                is_active
            )
            .fetch_all(&state.db_pool)
            .await
        }
    };

    rows.map(|rows| rows.into_iter().map(SymbolItem::from).collect())
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to fetch symbols from DB");
            ApiError::DataSourceInterrupted
        })
}

async fn fetch_last_synced_ms(state: &AppState) -> Option<i64> {
    sqlx::query_scalar!("SELECT MAX(updated_at_ms) FROM symbols")
        .fetch_one(&state.db_pool)
        .await
        .ok()
        .flatten()
}

// ── DB 行對應結構 ─────────────────────────────────────────────────────────────

struct SymbolRow {
    symbol: String,
    name: String,
    exchange: Exchange,
    data_source: String,
    earliest_ms: i64,
    latest_ms: i64,
    is_active: bool,
    updated_at_ms: i64,
}

impl From<SymbolRow> for SymbolItem {
    fn from(row: SymbolRow) -> Self {
        Self {
            symbol: row.symbol,
            name: row.name,
            exchange: row.exchange,
            data_source: row.data_source,
            earliest_available_ms: row.earliest_ms,
            latest_available_ms: row.latest_ms,
            is_active: row.is_active,
            updated_at_ms: row.updated_at_ms,
        }
    }
}
