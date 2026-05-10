/// src/data/symbol_sync.rs
///
/// 動態 Symbol 清單同步。
///
/// 在現有基礎上新增：
///   同步時一併寫入 finmind_earliest_ms，
///   供 manual_sync.rs detect_gaps() 計算歷史補齊的起點。
///
///   refresh_symbols_from_finmind()：
///   整合「取得 FinMind 股票清單 → upsert DB」的共用流程，
///   取代原本分散在 manual_sync::ensure_symbols_metadata
///   與 scheduler::sync_all_market_symbols 的重複邏輯。
use crate::constants::FINMIND_API_TOKEN_ENV;
use crate::data::fetch::{fetch_stock_info_map, StockInfo};
use crate::data::fetch_rate_limiter::FinMindRateLimiter;
use crate::data::models::current_timestamp_ms;
use crate::domain::BridgeError;
use crate::models::enums::{DataSource, Exchange, SymbolFetchScope};

use reqwest::Client;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Symbol 同步資料（從 FinMind 取得）。
#[derive(Debug)]
pub struct SymbolSyncData {
    pub symbol: String,
    pub name: String,
    pub exchange: Exchange,
    pub data_source: DataSource,
    /// FinMind 可提供的最早資料日期（毫秒）。
    /// 若 FinMind 無此資訊則為 None。
    pub finmind_earliest_ms: Option<i64>,
    pub latest_ms: i64,
    pub is_active: bool,
}

/// Symbol 同步結果摘要。
#[derive(Debug)]
pub struct SyncSummary {
    pub inserted: u32,
    pub updated: u32,
    pub failed: u32,
}

/// 將 Symbol 清單寫入 DB，包含 finmind_earliest_ms。
///
/// 使用 UPSERT（INSERT ... ON CONFLICT DO UPDATE）確保冪等性。
/// 下市標的 is_active = false，歷史資料保留。
pub async fn upsert_symbols(
    db_pool: &PgPool,
    symbols: &[SymbolSyncData],
    synced_at_ms: i64,
) -> Result<SyncSummary, BridgeError> {
    let mut inserted = 0u32;
    let mut updated = 0u32;
    let mut failed = 0u32;

    for symbol_data in symbols {
        let result = sqlx::query!(
            r#"
            INSERT INTO symbols (
                symbol, name, exchange, data_source,
                finmind_earliest_ms,
                earliest_ms, latest_ms,
                is_active, updated_at_ms
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (symbol) DO UPDATE SET
                name                  = EXCLUDED.name,
                exchange              = EXCLUDED.exchange,
                data_source           = EXCLUDED.data_source,
                -- finmind_earliest_ms 只有在 FinMind 回傳非 NULL 時才更新
                -- 避免後續同步因資料缺失而誤清除已知的最早日期
                finmind_earliest_ms   = COALESCE(
                                            EXCLUDED.finmind_earliest_ms,
                                            symbols.finmind_earliest_ms
                                        ),
                latest_ms             = EXCLUDED.latest_ms,
                is_active             = EXCLUDED.is_active,
                updated_at_ms         = EXCLUDED.updated_at_ms
            "#,
            symbol_data.symbol,
            symbol_data.name,
            symbol_data.exchange.to_string(),
            symbol_data.data_source.to_string(),
            symbol_data.finmind_earliest_ms,
            symbol_data.finmind_earliest_ms.unwrap_or(0),
            symbol_data.latest_ms,
            symbol_data.is_active,
            synced_at_ms,
        )
        .execute(db_pool)
        .await;

        match result {
            Ok(r) => {
                if r.rows_affected() == 1 {
                    inserted += 1;
                } else {
                    updated += 1;
                }
            }
            Err(e) => {
                error!(
                    error = %e,
                    symbol = %symbol_data.symbol,
                    "Failed to upsert symbol"
                );
                failed += 1;
            }
        }
    }

    info!(
        inserted = inserted,
        updated = updated,
        failed = failed,
        "Symbol sync complete"
    );

    Ok(SyncSummary {
        inserted,
        updated,
        failed,
    })
}

/// 查詢特定 symbol 的 finmind_earliest_ms。
/// 供 detect_gaps() 使用。
pub async fn get_finmind_earliest_ms(
    db_pool: &PgPool,
    symbol: &str,
) -> Result<Option<i64>, BridgeError> {
    let row = sqlx::query!(
        r#"
        SELECT COALESCE(finmind_earliest_ms, earliest_ms) AS finmind_earliest_ms
        FROM symbols
        WHERE symbol = $1
        "#,
        symbol,
    )
    .fetch_optional(db_pool)
    .await
    .map_err(|e| {
        error!(error = %e, symbol = %symbol, "Failed to query finmind_earliest_ms");
        BridgeError::FinMindDataSourceError {
            context: "Failed to query finmind_earliest_ms: ".into(),
            source: Some(Box::new(e)),
        }
    })?;

    Ok(row.and_then(|r| r.finmind_earliest_ms))
}

// ── refresh_symbols_from_finmind ──────────────────────────────────────────────

/// 從 FinMind TaiwanStockInfo 取得股票清單並 upsert 到 DB。
///
/// 統一取代原本分散的兩份重複邏輯：
///   - `manual_sync::ensure_symbols_metadata`（MissingOnly）
///   - `scheduler::sync_all_market_symbols`（AllMarkets）
///
/// # Returns
/// - `AllMarkets`：FinMind 回傳的全部 symbol（已排序）
/// - `MissingOnly`：原樣回傳傳入的清單（含原本已有 metadata 的）
pub async fn refresh_symbols_from_finmind(
    db_pool: &PgPool,
    http_client: &Client,
    rate_limiter: &Arc<FinMindRateLimiter>,
    scope: SymbolFetchScope<'_>,
) -> Result<Vec<String>, BridgeError> {
    match scope {
        // ── AllMarkets：全量打 API，全量 upsert ───────────────────────────────
        SymbolFetchScope::AllMarkets => {
            let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();

            rate_limiter.acquire().await.ok();
            let stock_info_map = fetch_stock_info_map(http_client, &api_token).await?;
            rate_limiter.mark_request_used().await;

            let mut all_symbols = stock_info_map.keys().cloned().collect::<Vec<_>>();
            all_symbols.sort();

            let now_ms = current_timestamp_ms();
            let payload = build_upsert_payload(&all_symbols, &stock_info_map, now_ms);
            let summary = upsert_symbols(db_pool, &payload, now_ms).await?;

            info!(
                inserted = summary.inserted,
                updated = summary.updated,
                failed = summary.failed,
                total = all_symbols.len(),
                "refresh_symbols_from_finmind(AllMarkets) complete"
            );

            Ok(all_symbols)
        }

        // ── MissingOnly：先查 DB，只補缺口 ───────────────────────────────────
        SymbolFetchScope::MissingOnly(symbols) => {
            if symbols.is_empty() {
                return Ok(vec![]);
            }

            let existing: HashSet<String> = sqlx::query_scalar!(
                "SELECT symbol FROM symbols WHERE symbol = ANY($1) AND name IS NOT NULL",
                &symbols as &[String]
            )
            .fetch_all(db_pool)
            .await
            .map_err(|e| {
                BridgeError::from_db(
                    "refresh_symbols_from_finmind: query existing symbols failed",
                    e,
                )
            })?
            .into_iter()
            .collect();

            let missing: Vec<String> = symbols
                .iter()
                .filter(|s| !existing.contains(*s))
                .cloned()
                .collect();

            if missing.is_empty() {
                info!(
                    count = symbols.len(),
                    "All symbols already have metadata, skipping FinMind fetch"
                );
                return Ok(symbols.to_vec());
            }

            info!(
                total = symbols.len(),
                missing = missing.len(),
                "Fetching metadata only for symbols missing from DB"
            );

            let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();

            rate_limiter.acquire().await.ok();
            let stock_info_map = fetch_stock_info_map(http_client, &api_token).await?;
            rate_limiter.mark_request_used().await;

            let now_ms = current_timestamp_ms();
            let payload = build_upsert_payload(&missing, &stock_info_map, now_ms);
            let summary = upsert_symbols(db_pool, &payload, now_ms).await?;

            info!(
                inserted = summary.inserted,
                updated = summary.updated,
                failed = summary.failed,
                total = missing.len(),
                "refresh_symbols_from_finmind(MissingOnly) complete"
            );

            Ok(symbols.to_vec())
        }
    }
}

// ── 內部工具 ──────────────────────────────────────────────────────────────────

/// symbol 清單 → SymbolSyncData payload，供兩個 variant 共用。
fn build_upsert_payload(
    symbols: &[String],
    stock_info_map: &HashMap<String, StockInfo>,
    now_ms: i64,
) -> Vec<SymbolSyncData> {
    symbols
        .iter()
        .map(|symbol| {
            let info = stock_info_map.get(symbol);

            if info.is_none() {
                warn!(
                    symbol = %symbol,
                    "Symbol not found in FinMind TaiwanStockInfo, marking as inactive"
                );
            }

            SymbolSyncData {
                symbol: symbol.clone(),
                name: info
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| format!("股票 {}", symbol)),
                exchange: info.map(|s| s.exchange).unwrap_or(Exchange::Twse),
                data_source: DataSource::FinMind,
                finmind_earliest_ms: None,
                latest_ms: now_ms,
                is_active: info.map(|s| s.is_active).unwrap_or(false),
            }
        })
        .collect()
}

/// 查詢 is_active = true 的 symbol 清單。
///
/// * `filter` — `Some(&[...])` 從指定清單中過濾（manual_sync 用）
///              `None` 回傳全部上市 symbol（scheduler 用）
pub async fn fetch_active_symbols(
    db_pool: &PgPool,
    filter: Option<&[String]>,
) -> Result<Vec<String>, BridgeError> {
    let symbols = match filter {
        Some(targets) => {
            sqlx::query_scalar!(
                "SELECT symbol FROM symbols WHERE symbol = ANY($1) AND is_active = true",
                targets as &[String]
            )
            .fetch_all(db_pool)
            .await
        }
        None => {
            sqlx::query_scalar!("SELECT symbol FROM symbols WHERE is_active = true")
                .fetch_all(db_pool)
                .await
        }
    }
    .map_err(|e| BridgeError::from_db("fetch_active_symbols failed", e))?;

    Ok(symbols)
}
