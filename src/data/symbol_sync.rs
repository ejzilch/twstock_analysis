/// src/data/symbol_sync.rs
///
/// 動態 Symbol 清單同步。
///
/// 在現有基礎上新增：
///   同步時一併寫入 finmind_earliest_ms，
///   供 manual_sync.rs detect_gaps() 計算歷史補齊的起點。
use sqlx::PgPool;
use tracing::{error, info, warn};

use crate::core::BridgeError;
use crate::models::enums::{DataSource, Exchange};

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
                latest_ms   = EXCLUDED.latest_ms,
                is_active             = EXCLUDED.is_active,
                updated_at_ms         = EXCLUDED.updated_at_ms
            "#,
            symbol_data.symbol,
            symbol_data.name,
            symbol_data.exchange.to_string(),
            symbol_data.data_source.to_string(),
            symbol_data.finmind_earliest_ms,
            // earliest_available_ms 與 finmind_earliest_ms 相同來源
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

/// Symbol 同步結果摘要。
#[derive(Debug)]
pub struct SyncSummary {
    pub inserted: u32,
    pub updated: u32,
    pub failed: u32,
}
