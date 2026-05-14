use crate::data::dataset_sync::{detect_gaps_by_date, DatasetSync, DatasetSyncResult, SyncContext};
use crate::data::manual_sync::{DateRange, SyncScope};
use crate::data::symbol_sync::get_finmind_earliest_ms;
use crate::domain::BridgeError;
use crate::models::{DatasetType, DateColumnType, Interval};

use async_trait::async_trait;
use chrono::{DateTime, Local, Months};
use tracing::warn;

pub struct CandlesDataset {
    pub interval: Interval,
}

impl Default for CandlesDataset {
    fn default() -> Self {
        Self {
            interval: Interval::OneDay,
        }
    }
}

#[async_trait]
impl DatasetSync for CandlesDataset {
    fn name(&self) -> &str {
        DatasetType::TaiwanStockPrice.as_finmind_str()
    }

    async fn detect_gaps(
        &self,
        scope: &SyncScope,
        symbol: &str,
        ctx: &SyncContext<'_>,
    ) -> Result<(Option<DateRange>, Option<DateRange>), BridgeError> {
        let finmind_earliest_ms = get_finmind_earliest_ms(ctx.db_pool, symbol).await?;
        let today = Local::now().date_naive();
        let finmind_earliest = finmind_earliest_ms
            .map(|ms| {
                let secs = ms / 1000;
                DateTime::from_timestamp(secs, 0)
                    .unwrap_or_default()
                    .date_naive()
            })
            .unwrap_or_else(|| {
                warn!(symbol = %symbol, "finmind_earliest_ms is NULL, using fallback");
                today
                    .checked_sub_months(Months::new(5 * 12))
                    .expect("日期計算超出範圍")
            });

        detect_gaps_by_date(
            ctx.db_pool,
            symbol,
            "candles",
            "timestamp_ms",
            DateColumnType::TimestampMs,
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
        // 直接委派給現有的 fetch_and_insert_gap()
        // 回傳值轉換為 DatasetSyncResult
        let (inserted, skipped, failed) = crate::data::manual_sync::fetch_and_insert_gap(
            ctx.db_pool,
            ctx.http_client,
            ctx.rate_limiter,
            ctx.buffer,
            ctx.redis,
            ctx.sync_id,
            symbol,
            self.interval,
            gap,
        )
        .await?;
        Ok(DatasetSyncResult {
            inserted,
            skipped,
            failed,
        })
    }
}
