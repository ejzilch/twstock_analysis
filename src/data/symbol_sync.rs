use crate::data::models::DataSource;
use anyhow::Context;
use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashSet;

// 內部使用的 JSON 接收結構
#[derive(serde::Deserialize, Debug)]
struct FinMindInfoResponse {
    data: Vec<FinMindStockInfo>,
}

#[derive(serde::Deserialize, Debug)]
struct FinMindStockInfo {
    pub stock_id: String,
    pub stock_name: String,
    // 使用 rename 對接 API 的 "type" 關鍵字
    #[serde(rename = "type")]
    pub stock_type: String,
}

pub struct SymbolSyncer {
    client: reqwest::Client,
    pool: PgPool,
}

impl SymbolSyncer {
    pub fn new(pool: PgPool) -> Self {
        Self {
            client: reqwest::Client::new(),
            pool,
        }
    }

    /// 執行完整清單同步，處理新增與下市邏輯
    pub async fn sync_symbols(&self) -> anyhow::Result<()> {
        let url = "https://api.finmindtrade.com/api/v4/data?dataset=TaiwanStockInfo";

        let response: FinMindInfoResponse = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to fetch stock info from FinMind")?
            .json()
            .await
            .context("Failed to parse stock info JSON")?;

        let valid_stocks: Vec<&FinMindStockInfo> = response
            .data
            .iter()
            .filter(|s| s.stock_type == "twse" || s.stock_type == "tpex")
            .collect();

        if valid_stocks.is_empty() {
            tracing::warn!("No valid stocks found from API, skipping sync");
            return Ok(());
        }

        // 提取所有有效的 symbol 為字串陣列，供後續批次比對使用
        let fetched_symbols_vec: Vec<String> =
            valid_stocks.iter().map(|s| s.stock_id.clone()).collect();

        let mut transaction = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction")?;

        let now_ms = Utc::now().timestamp_millis();
        let source_str = DataSource::FinMind.to_string();

        // 1. 批次寫入或更新現有標的 (Bulk Upsert)
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO symbols (symbol, name, exchange, data_source, is_active, updated_at_ms) ",
        );

        query_builder.push_values(&valid_stocks, |mut b, stock| {
            b.push_bind(&stock.stock_id)
                .push_bind(&stock.stock_name)
                .push_bind(stock.stock_type.to_uppercase())
                .push_bind(&source_str)
                .push_bind(true)
                .push_bind(now_ms);
        });

        query_builder.push(
            " ON CONFLICT (symbol) DO UPDATE SET \
            name = EXCLUDED.name, \
            exchange = EXCLUDED.exchange, \
            is_active = true, \
            updated_at_ms = EXCLUDED.updated_at_ms",
        );

        query_builder
            .build()
            .execute(&mut *transaction)
            .await
            .context("Failed to execute bulk upsert for symbols")?;

        // 2. 批次標記下市標的 (Single Query)
        // 找出原本在 DB 是 active，但這次 API 沒有回傳的標的
        let rows_affected = sqlx::query(
            "UPDATE symbols \
             SET is_active = false, updated_at_ms = $1 \
             WHERE is_active = true AND NOT (symbol = ANY($2))",
        )
        .bind(now_ms)
        .bind(&fetched_symbols_vec)
        .execute(&mut *transaction)
        .await
        .context("Failed to mark inactive symbols")?
        .rows_affected();

        if rows_affected > 0 {
            tracing::info!(inactive_count = rows_affected, "Marked symbols as inactive");
        }

        transaction
            .commit()
            .await
            .context("Failed to commit symbol sync transaction")?;

        tracing::info!(
            synced_count = fetched_symbols_vec.len(),
            "Symbol sync completed successfully"
        );

        Ok(())
    }
}
