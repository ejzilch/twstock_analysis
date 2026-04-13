use crate::data::models::DataSource;
use anyhow::Context;
use chrono::Utc;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::time::Duration;

// FinMind TaiwanStockInfo API timeout
const FINMIND_TIMEOUT_SECS: u64 = 15;

// ── 內部反序列化結構（非對外公開）────────────────────────────────────────────

#[derive(serde::Deserialize, Debug)]
struct FinMindInfoResponse {
    data: Vec<FinMindStockInfo>,
}

#[derive(serde::Deserialize, Debug)]
struct FinMindStockInfo {
    pub stock_id: String,
    pub stock_name: String,
    // FinMind 以 "type" 回傳交易所類型，為 Rust 關鍵字故使用 rename
    #[serde(rename = "type")]
    pub stock_type: String, // "twse" / "tpex"
}

// ── 公開型別 ─────────────────────────────────────────────────────────────────

/// 動態 Symbol 清單同步器
///
/// 每日 02:00 排程呼叫 sync_symbols()，從 FinMind TaiwanStockInfo 取得最新清單。
/// 新增標的寫入並標記 is_active = true。
/// 下市標的標記 is_active = false，保留歷史資料不刪除。
pub struct SymbolSyncer {
    client: reqwest::Client,
    pool: PgPool,
}

impl SymbolSyncer {
    /// 建立新的 SymbolSyncer，reqwest Client 設定 15s timeout
    pub fn new(pool: PgPool) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(FINMIND_TIMEOUT_SECS))
            .build()
            .expect("Failed to build reqwest Client for SymbolSyncer");

        Self { client, pool }
    }

    /// 執行完整 Symbol 清單同步
    ///
    /// 同步流程：
    /// 1. 從 FinMind 取得最新股票清單，過濾 TWSE / TPEX 標的
    /// 2. Bulk Upsert：新增或更新現有標的，is_active = true
    /// 3. 批次標記下市標的：本次 API 未回傳的 active 標的設為 is_active = false
    /// 4. COMMIT 事務，寫入 tracing log
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
            .context("Failed to parse FinMind stock info JSON")?;

        // 過濾只保留 TWSE / TPEX 標的，排除其他類型
        let valid_stocks: Vec<&FinMindStockInfo> = response
            .data
            .iter()
            .filter(|s| s.stock_type == "twse" || s.stock_type == "tpex")
            .collect();

        if valid_stocks.is_empty() {
            tracing::warn!(
                "FinMind returned no valid TWSE/TPEX stocks, skipping sync to avoid data loss"
            );
            return Ok(());
        }

        let fetched_symbol_ids: Vec<String> =
            valid_stocks.iter().map(|s| s.stock_id.clone()).collect();

        let now_ms = Utc::now().timestamp_millis();
        let source_str = DataSource::FinMind.to_string();

        let mut transaction = self
            .pool
            .begin()
            .await
            .context("Failed to begin symbol sync transaction")?;

        // 步驟 1: Bulk Upsert，新增或更新標的
        // exchange 以 to_uppercase() 轉換 "twse" -> "TWSE", "tpex" -> "TPEX"
        // 對應 API_CONTRACT.md 與 init_schema.sql 的 exchange 欄位格式
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO symbols \
             (symbol, name, exchange, data_source, is_active, updated_at_ms) ",
        );

        query_builder.push_values(&valid_stocks, |mut b, stock| {
            b.push_bind(&stock.stock_id)
                .push_bind(&stock.stock_name)
                .push_bind(stock.stock_type.to_uppercase()) // "twse" -> "TWSE"
                .push_bind(&source_str)
                .push_bind(true)
                .push_bind(now_ms);
        });

        query_builder.push(
            " ON CONFLICT (symbol) DO UPDATE SET \
             name          = EXCLUDED.name,          \
             exchange      = EXCLUDED.exchange,      \
             is_active     = true,                   \
             updated_at_ms = EXCLUDED.updated_at_ms",
        );

        query_builder
            .build()
            .execute(&mut *transaction)
            .await
            .context("Failed to execute bulk upsert for symbols")?;

        // 步驟 2: 批次標記下市標的
        // 找出原本 is_active = true 但本次 API 未回傳的標的，標記為下市
        let delisted_count = sqlx::query(
            "UPDATE symbols \
             SET is_active = false, updated_at_ms = $1 \
             WHERE is_active = true AND NOT (symbol = ANY($2))",
        )
        .bind(now_ms)
        .bind(&fetched_symbol_ids)
        .execute(&mut *transaction)
        .await
        .context("Failed to mark delisted symbols as inactive")?
        .rows_affected();

        if delisted_count > 0 {
            tracing::info!(
                delisted_count = delisted_count,
                "Marked symbols as inactive (delisted)"
            );
        }

        // 步驟 3: COMMIT
        transaction
            .commit()
            .await
            .context("Failed to commit symbol sync transaction")?;

        tracing::info!(
            synced_count = fetched_symbol_ids.len(),
            delisted_count = delisted_count,
            "Symbol sync completed successfully"
        );

        Ok(())
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_uppercase_conversion() {
        // 確認 FinMind stock_type 轉換為 DB 欄位格式的邏輯正確
        assert_eq!("twse".to_uppercase(), "TWSE");
        assert_eq!("tpex".to_uppercase(), "TPEX");
    }

    #[test]
    fn test_filter_excludes_non_twse_tpex() {
        let stocks = vec![
            FinMindStockInfo {
                stock_id: "2330".to_string(),
                stock_name: "台積電".to_string(),
                stock_type: "twse".to_string(),
            },
            FinMindStockInfo {
                stock_id: "6547".to_string(),
                stock_name: "高端疫苗".to_string(),
                stock_type: "tpex".to_string(),
            },
            FinMindStockInfo {
                stock_id: "0000".to_string(),
                stock_name: "其他類型".to_string(),
                stock_type: "other".to_string(),
            },
        ];

        let valid: Vec<&FinMindStockInfo> = stocks
            .iter()
            .filter(|s| s.stock_type == "twse" || s.stock_type == "tpex")
            .collect();

        assert_eq!(valid.len(), 2);
        assert!(valid.iter().all(|s| s.stock_type != "other"));
    }
}
