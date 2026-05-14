use crate::constants::{
    FINMIND_API_TOKEN_ENV, FINMIND_DATE_FORMAT, REDIS_TRADING_DATES_KEY,
    REDIS_TRADING_DATES_TTL_SECS,
};
use crate::data::dataset_sync::{DatasetSync, DatasetSyncResult, SyncContext};
use crate::data::fetch::fetch_trading_dates;
use crate::data::manual_sync::DateRange;
use crate::domain::BridgeError;
use crate::models::DatasetType;
use async_trait::async_trait;
use redis::AsyncCommands;
use tracing::info;

pub struct TradingDateDataset;

#[async_trait]
impl DatasetSync for TradingDateDataset {
    fn name(&self) -> &str {
        DatasetType::TaiwanStockTradingDate.as_finmind_str()
    }

    // detect_gaps 使用預設實作（全範圍），直接刷新快取

    async fn fetch_and_insert(
        &self,
        _symbol: &str,
        gap: &DateRange,
        ctx: &mut SyncContext<'_>,
    ) -> Result<DatasetSyncResult, BridgeError> {
        let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();
        let from_str = gap.from_date.format(FINMIND_DATE_FORMAT).to_string();
        let to_str = gap.to_date.format(FINMIND_DATE_FORMAT).to_string();

        ctx.rate_limiter.acquire().await;
        let dates = fetch_trading_dates(ctx.http_client, &api_token, &from_str, &to_str).await?;
        ctx.rate_limiter.mark_request_used().await;

        // 更新 Redis 快取
        let mut vec_days: Vec<String> = dates
            .iter()
            .map(|d| d.format(FINMIND_DATE_FORMAT).to_string())
            .collect();
        vec_days.sort();

        if let Ok(payload) = serde_json::to_string(&vec_days) {
            let _: redis::RedisResult<()> = ctx
                .redis
                .set_ex(
                    REDIS_TRADING_DATES_KEY,
                    payload,
                    REDIS_TRADING_DATES_TTL_SECS,
                )
                .await;
        }

        info!(count = dates.len(), "TradingDate cache refreshed");
        Ok(DatasetSyncResult {
            inserted: dates.len() as i32,
            skipped: 0,
            failed: 0,
        })
    }
}
