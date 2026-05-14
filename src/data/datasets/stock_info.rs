use crate::data::dataset_sync::{DatasetSync, DatasetSyncResult, SyncContext};
use crate::data::manual_sync::DateRange;
use crate::data::symbol_sync::refresh_symbols_from_finmind;
use crate::domain::BridgeError;
use crate::models::enums::{DatasetType, SymbolFetchScope};
use async_trait::async_trait;

pub struct StockInfoDataset;

#[async_trait]
impl DatasetSync for StockInfoDataset {
    fn name(&self) -> &str {
        DatasetType::TaiwanStockInfo.as_finmind_str()
    }

    // detect_gaps 使用預設實作（全範圍），因為 StockInfo 是全量覆蓋

    async fn fetch_and_insert(
        &self,
        _symbol: &str,
        _gap: &DateRange,
        ctx: &mut SyncContext<'_>,
    ) -> Result<DatasetSyncResult, BridgeError> {
        // StockInfo 是全市場一次抓，symbol 參數忽略
        refresh_symbols_from_finmind(
            ctx.db_pool,
            ctx.http_client,
            ctx.rate_limiter,
            SymbolFetchScope::AllMarkets,
        )
        .await?;

        Ok(DatasetSyncResult {
            inserted: 0,
            skipped: 0,
            failed: 0,
        })
    }
}
