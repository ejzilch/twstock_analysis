use crate::constants::{FINMIND_API_TOKEN_ENV, FINMIND_DATE_FORMAT};
use crate::data::dataset_sync::{detect_gaps_by_date, DatasetSync, DatasetSyncResult, SyncContext};
use crate::data::manual_sync::{DateRange, SyncScope};
use crate::data::models::current_timestamp_ms;
use crate::domain::BridgeError;
use crate::models::enums::{DatasetType, DateColumnType};

use async_trait::async_trait;
use chrono::{Local, Months, NaiveDate};
use serde::Deserialize;
use tracing::{error, info, warn};

pub struct InstitutionalInvestorsDataset;

/// FinMind TaiwanStockInstitutionalInvestorsBuySell 單筆原始資料
#[derive(Debug, Deserialize)]
struct RawInstitutionalRow {
    date: String,
    #[serde(rename = "stock_id")]
    _stock_id: String,
    buy: i64,
    sell: i64,
    name: String,
}

/// 寬表一天的聚合資料（pivot 後）
#[derive(Debug, Default)]
struct DayRow {
    foreign_investor_buy: i64,
    foreign_investor_sell: i64,
    investment_trust_buy: i64,
    investment_trust_sell: i64,
    dealer_self_buy: i64,
    dealer_self_sell: i64,
    dealer_hedging_buy: i64,
    dealer_hedging_sell: i64,
    foreign_dealer_self_buy: i64,
    foreign_dealer_self_sell: i64,
}

#[async_trait]
impl DatasetSync for InstitutionalInvestorsDataset {
    fn name(&self) -> &str {
        DatasetType::TaiwanStockInstitutionalInvestorsBuySell.as_finmind_str()
    }

    async fn detect_gaps(
        &self,
        scope: &SyncScope,
        symbol: &str,
        ctx: &SyncContext<'_>,
    ) -> Result<(Option<DateRange>, Option<DateRange>), BridgeError> {
        let today = Local::now().date_naive();
        let finmind_earliest = scope.from_date.unwrap_or_else(|| {
            today
                .checked_sub_months(Months::new(5 * 12))
                .expect("日期計算超出範圍")
        });

        detect_gaps_by_date(
            ctx.db_pool,
            symbol,
            "institutional_investors",
            "date",
            DateColumnType::Date,
            scope,
            ctx.trading_dates,
            finmind_earliest,
        )
        .await
    }

    async fn fetch_and_insert(
        &self,
        symbol: &str,
        gap: &DateRange,
        ctx: &mut SyncContext<'_>,
    ) -> Result<DatasetSyncResult, BridgeError> {
        let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();
        let from_str = gap.from_date.to_string();
        let to_str = gap.to_date.to_string();

        ctx.rate_limiter.acquire().await;

        let rows = fetch_institutional_raw(ctx.http_client, symbol, &from_str, &to_str, &api_token)
            .await?;

        ctx.rate_limiter.mark_request_used().await;

        let day_map = pivot_to_wide(rows);
        let now_ms = current_timestamp_ms();
        let mut inserted = 0i32;
        let mut skipped = 0i32;

        for (date, row) in &day_map {
            let result = sqlx::query!(
                r#"
                INSERT INTO institutional_investors (
                    symbol, date,
                    foreign_investor_buy, foreign_investor_sell,
                    investment_trust_buy, investment_trust_sell,
                    dealer_self_buy, dealer_self_sell,
                    dealer_hedging_buy, dealer_hedging_sell,
                    foreign_dealer_self_buy, foreign_dealer_self_sell,
                    created_at_ms
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
                ON CONFLICT (symbol, date) DO NOTHING
                "#,
                symbol,
                date,
                row.foreign_investor_buy,
                row.foreign_investor_sell,
                row.investment_trust_buy,
                row.investment_trust_sell,
                row.dealer_self_buy,
                row.dealer_self_sell,
                row.dealer_hedging_buy,
                row.dealer_hedging_sell,
                row.foreign_dealer_self_buy,
                row.foreign_dealer_self_sell,
                now_ms,
            )
            .execute(ctx.db_pool)
            .await
            .map_err(|e| {
                error!(error = %e, symbol = %symbol, "Failed to insert institutional_investors row");
                BridgeError::from_db("institutional_investors insert failed", e)
            })?;

            if result.rows_affected() > 0 {
                inserted += 1;
            } else {
                skipped += 1;
            }
        }

        info!(
            symbol = %symbol,
            from   = %from_str,
            to     = %to_str,
            inserted,
            skipped,
            "InstitutionalInvestors fetch_and_insert complete"
        );

        Ok(DatasetSyncResult {
            inserted,
            skipped,
            failed: 0,
        })
    }
}

/// 呼叫 FinMind API 取得原始長表資料
async fn fetch_institutional_raw(
    client: &reqwest::Client,
    symbol: &str,
    from_date: &str,
    to_date: &str,
    api_token: &str,
) -> Result<Vec<RawInstitutionalRow>, BridgeError> {
    use crate::constants::{FINMIND_API_BASE_URL, FINMIND_API_TIMEOUT_SECS};

    let base = std::env::var(FINMIND_API_BASE_URL).unwrap_or_default();
    let url = format!(
        "{base}/data?dataset=TaiwanStockInstitutionalInvestorsBuySell\
         &data_id={symbol}&start_date={from_date}&end_date={to_date}&token={api_token}"
    );

    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(FINMIND_API_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| BridgeError::FinMindDataSourceError {
            context: "InstitutionalInvestors request failed".into(),
            source: Some(Box::new(e)),
        })?;

    #[derive(Deserialize)]
    struct FinMindResp {
        status: u32,
        #[serde(default)]
        data: Vec<RawInstitutionalRow>,
    }

    let body: FinMindResp = resp
        .json()
        .await
        .map_err(|e| BridgeError::FinMindDataSourceError {
            context: "InstitutionalInvestors deserialize failed".into(),
            source: Some(Box::new(e)),
        })?;

    if body.status != 200 {
        return Err(BridgeError::FinMindDataSourceError {
            context: format!("FinMind status={}", body.status),
            source: None,
        });
    }

    Ok(body.data)
}

/// 長表 pivot 成寬表（以 date 為 key）
fn pivot_to_wide(rows: Vec<RawInstitutionalRow>) -> std::collections::HashMap<NaiveDate, DayRow> {
    use std::collections::HashMap;

    let mut map: HashMap<NaiveDate, DayRow> = HashMap::new();

    for row in rows {
        let Ok(date) = NaiveDate::parse_from_str(&row.date, FINMIND_DATE_FORMAT) else {
            warn!(date = %row.date, "Invalid date in institutional investors row, skipping");
            continue;
        };
        let entry = map.entry(date).or_default();
        match row.name.as_str() {
            "Foreign_Investor" => {
                entry.foreign_investor_buy += row.buy;
                entry.foreign_investor_sell += row.sell;
            }
            "Investment_Trust" => {
                entry.investment_trust_buy += row.buy;
                entry.investment_trust_sell += row.sell;
            }
            "Dealer_self" => {
                entry.dealer_self_buy += row.buy;
                entry.dealer_self_sell += row.sell;
            }
            "Dealer_Hedging" => {
                entry.dealer_hedging_buy += row.buy;
                entry.dealer_hedging_sell += row.sell;
            }
            "Foreign_Dealer_Self" => {
                entry.foreign_dealer_self_buy += row.buy;
                entry.foreign_dealer_self_sell += row.sell;
            }
            other => warn!(name = %other, "Unknown investor type, skipping"),
        }
    }

    map
}
