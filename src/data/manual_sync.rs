/// 手動補資料核心邏輯。
///
/// 流程：
///   1. detect_gaps()    — 查 DB MIN/MAX，計算缺口 A（歷史段）和缺口 B（近期段）
///   2. fetch_and_insert_gap() — 對缺口分批請求 FinMind，INSERT ON CONFLICT DO NOTHING
///   3. RateLimitQueue   — 由 FinMindRateLimiter.acquire() 管理，達上限自動等待
use crate::constants::{
    FINMIND_API_TOKEN_ENV, FINMIND_DATE_FORMAT, FINMIND_ROW_LIMIT, MANUAL_SYNC_BATCH_DAYS,
    REDIS_TRADING_DATES_KEY, REDIS_TRADING_DATES_TTL_SECS,
};
use crate::data::dataset_sync::{DatasetSync, SyncContext};
use crate::data::datasets::{
    candles::CandlesDataset, institutional_investors::InstitutionalInvestorsDataset,
    stock_info::StockInfoDataset, trading_date::TradingDateDataset,
};
use crate::data::db::{sync_log_update_counts, sync_log_update_status, BulkInsertBuffer};
use crate::data::fetch::{fetch_range, fetch_trading_dates};
use crate::data::fetch_rate_limiter::FinMindRateLimiter;
use crate::data::implementations::{PostgresDbWriter, RedisInvalidator};
use crate::data::models::current_timestamp_ms;
use crate::data::symbol_sync::{fetch_active_symbols, refresh_symbols_from_finmind};
use crate::domain::BridgeError;
use crate::models::enums::{DatasetType, Interval, SymbolFetchScope, SymbolSyncStatus, SyncStatus};
use crate::services::sync_state::{
    is_sync_cancel_requested, update_symbol_progress, update_sync_status,
};
use crate::services::sync_types::GapProgress;

use chrono::{Datelike, Duration, Local, Months, NaiveDate};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use reqwest::Client;
use sqlx::PgPool;
use std::{collections::HashSet, sync::Arc, time::Instant};
use tracing::{error, info, warn};

// ── 所有支援的 K 線粒度 ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SyncScope {
    pub full_sync: bool,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
pub struct DateRange {
    pub from_date: NaiveDate,
    pub to_date: NaiveDate,
}

impl DateRange {
    pub fn into_monthly_batches(self) -> Vec<DateRange> {
        let mut batches = Vec::new();
        let mut current = self.from_date;

        while current <= self.to_date {
            let batch_end =
                (current + Duration::days(MANUAL_SYNC_BATCH_DAYS as i64 - 1)).min(self.to_date);
            batches.push(DateRange {
                from_date: current,
                to_date: batch_end,
            });
            current = batch_end + Duration::days(1);
        }

        batches
    }
}

// ── fetch_and_insert_gap ──────────────────────────────────────────────────────

pub(crate) async fn fetch_and_insert_gap(
    db_pool: &PgPool,
    http_client: &Client,
    rate_limiter: &Arc<FinMindRateLimiter>,
    buffer: &mut BulkInsertBuffer,
    redis: &mut MultiplexedConnection,
    sync_id: &str,
    symbol: &str,
    interval: Interval,
    gap: &DateRange,
) -> Result<(i32, i32, i32), BridgeError> {
    let gap_started = Instant::now();
    let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();
    let writer = PostgresDbWriter::new(db_pool.clone());
    let mut invalidator = RedisInvalidator::new(redis.clone());

    let initial_inserted = buffer.total_inserted();
    let initial_skipped = buffer.total_skipped();

    info!(
        sync_id   = %sync_id,
        symbol    = %symbol,
        interval  = %interval,
        from_date = %gap.from_date,
        to_date   = %gap.to_date,
        "Starting gap fetch (single-request mode)"
    );

    // ── 取消檢查 ──────────────────────────────────────────────────────────────
    if is_sync_cancel_requested(redis, sync_id)
        .await
        .unwrap_or(false)
    {
        warn!(sync_id = %sync_id, symbol = %symbol, "Sync cancel requested, stopping");
        return Err(BridgeError::internal("SYNC_CANCELLED"));
    }

    // ── Rate limit 狀態更新 ───────────────────────────────────────────────────
    if let Some(resume_at_ms) = rate_limiter.predicted_resume_at_ms().await {
        let _ = update_sync_status(redis, sync_id, SyncStatus::RateLimitWaiting).await;
        info!(
            sync_id      = %sync_id,
            symbol       = %symbol,
            resume_at_ms = resume_at_ms,
            "Rate limit reached; waiting for next window"
        );
    }

    rate_limiter.acquire().await;
    let _ = update_sync_status(redis, sync_id, SyncStatus::Running).await;

    // ── 第一次嘗試：單次請求完整範圍 ─────────────────────────────────────────
    let from_str = gap.from_date.to_string();
    let to_str = gap.to_date.to_string();

    let candles = match fetch_range(
        http_client,
        &symbol,
        interval,
        &from_str,
        &to_str,
        &api_token,
    )
    .await
    {
        Ok(c) => {
            rate_limiter.mark_request_used().await;
            info!(
                sync_id  = %sync_id,
                symbol   = %symbol,
                interval = %interval.as_str(),
                from     = %from_str,
                to       = %to_str,
                count    = c.len(),
                "Single-request fetch complete"
            );
            c
        }
        Err(e) => {
            error!(
                error    = %e,
                sync_id  = %sync_id,
                symbol   = %symbol,
                interval = %interval.as_str(),
                from     = %from_str,
                to       = %to_str,
                "Single-request fetch failed"
            );
            sync_log_update_counts(db_pool, sync_id, 0, 0, 1)
                .await
                .unwrap_or_else(|e| warn!(error = %e, "sync_log update failed"));
            return Ok((0, 0, 1));
        }
    };

    // ── 截斷偵測：若回傳筆數達閾值，改用分批補抓 ─────────────────────────────
    //
    // FinMind 免費版單次回傳上限約 5000 筆。
    // 5年日K ≈ 1250 筆，正常不會觸發；
    // 分鐘K或超長範圍才需要 fallback。
    if candles.len() >= FINMIND_ROW_LIMIT {
        warn!(
            sync_id  = %sync_id,
            symbol   = %symbol,
            interval = %interval.as_str(),
            count    = candles.len(),
            threshold = FINMIND_ROW_LIMIT,
            "Response may be truncated, falling back to monthly batches"
        );
        return fetch_and_insert_gap_batched(
            db_pool,
            http_client,
            rate_limiter,
            buffer,
            redis,
            sync_id,
            symbol,
            interval,
            gap,
            &api_token,
        )
        .await;
    }

    // ── 正常路徑：寫入 buffer ─────────────────────────────────────────────────
    for candle in candles {
        buffer
            .push(candle, &writer, &mut invalidator)
            .await
            .map_err(|e| {
                error!(error = %e, sync_id = %sync_id, "Failed to push candle to buffer");
                e
            })?;
    }

    let flush_started = Instant::now();
    buffer.flush(&writer, &mut invalidator).await?;
    let flush_elapsed_ms = flush_started.elapsed().as_millis() as u64;

    let actual_inserted = buffer.total_inserted() - initial_inserted;
    let actual_skipped = buffer.total_skipped() - initial_skipped;

    if actual_inserted > 0 || actual_skipped > 0 {
        sync_log_update_counts(db_pool, sync_id, actual_inserted, actual_skipped, 0)
            .await
            .unwrap_or_else(|e| warn!(error = %e, "sync_log update failed"));
    }

    info!(
        sync_id      = %sync_id,
        symbol       = %symbol,
        interval     = %interval.as_str(),
        from_date    = %gap.from_date,
        to_date      = %gap.to_date,
        inserted     = actual_inserted,
        skipped      = actual_skipped,
        flush_ms     = flush_elapsed_ms,
        elapsed_ms   = gap_started.elapsed().as_millis() as u64,
        "Gap fetch complete (single-request)"
    );

    Ok((actual_inserted, actual_skipped, 0))
}

// ── fallback：分批補抓（截斷時才走這條路）────────────────────────────────────

pub(crate) async fn fetch_and_insert_gap_batched(
    db_pool: &PgPool,
    http_client: &Client,
    rate_limiter: &Arc<FinMindRateLimiter>,
    buffer: &mut BulkInsertBuffer,
    redis: &mut MultiplexedConnection,
    sync_id: &str,
    symbol: &str,
    interval: Interval,
    gap: &DateRange,
    api_token: &str,
) -> Result<(i32, i32, i32), BridgeError> {
    let gap_started = Instant::now();
    let writer = PostgresDbWriter::new(db_pool.clone());
    let mut invalidator = RedisInvalidator::new(redis.clone());

    let initial_inserted = buffer.total_inserted();
    let initial_skipped = buffer.total_skipped();

    let batches = gap.clone().into_monthly_batches();
    let total_batches = batches.len();
    let mut total_failed = 0i32;

    info!(
        sync_id   = %sync_id,
        symbol    = %symbol,
        interval  = %interval.as_str(),
        from_date = %gap.from_date,
        to_date   = %gap.to_date,
        batches   = total_batches,
        "Starting gap fetch (batched fallback)"
    );

    for (i, batch) in batches.iter().enumerate() {
        let batch_started = Instant::now();

        if is_sync_cancel_requested(redis, sync_id)
            .await
            .unwrap_or(false)
        {
            warn!(sync_id = %sync_id, symbol = %symbol, "Sync cancel requested, stopping");
            return Err(BridgeError::internal("SYNC_CANCELLED"));
        }

        if let Some(resume_at_ms) = rate_limiter.predicted_resume_at_ms().await {
            let _ = update_sync_status(redis, sync_id, SyncStatus::RateLimitWaiting).await;
            info!(
                sync_id      = %sync_id,
                symbol       = %symbol,
                resume_at_ms = resume_at_ms,
                "Rate limit reached; waiting for next window"
            );
        }

        rate_limiter.acquire().await;
        let _ = update_sync_status(redis, sync_id, SyncStatus::Running).await;

        let from_str = batch.from_date.to_string();
        let to_str = batch.to_date.to_string();

        match fetch_range(
            http_client,
            &symbol,
            interval,
            &from_str,
            &to_str,
            api_token,
        )
        .await
        {
            Ok(candles) => {
                rate_limiter.mark_request_used().await;
                let fetched = candles.len() as i32;

                for candle in candles {
                    buffer
                        .push(candle, &writer, &mut invalidator)
                        .await
                        .map_err(|e| {
                            error!(error = %e, sync_id = %sync_id, "Failed to push candle to buffer");
                            e
                        })?;
                }

                info!(
                    sync_id    = %sync_id,
                    symbol     = %symbol,
                    batch      = format!("{}/{}", i + 1, total_batches),
                    fetched    = fetched,
                    elapsed_ms = batch_started.elapsed().as_millis() as u64,
                    "Batch fetched and pushed to buffer"
                );
            }
            Err(e) => {
                error!(
                    error    = %e,
                    sync_id  = %sync_id,
                    symbol   = %symbol,
                    interval = %interval.as_str(),
                    from     = %from_str,
                    to       = %to_str,
                    "Batch fetch failed"
                );
                total_failed += 1;
                sync_log_update_counts(db_pool, sync_id, 0, 0, 1)
                    .await
                    .unwrap_or_else(|e| warn!(error = %e, "sync_log update failed"));
            }
        }
    }

    let flush_started = Instant::now();
    buffer.flush(&writer, &mut invalidator).await?;
    let flush_elapsed_ms = flush_started.elapsed().as_millis() as u64;

    let actual_inserted = buffer.total_inserted() - initial_inserted;
    let actual_skipped = buffer.total_skipped() - initial_skipped;

    if actual_inserted > 0 || actual_skipped > 0 {
        sync_log_update_counts(db_pool, sync_id, actual_inserted, actual_skipped, 0)
            .await
            .unwrap_or_else(|e| warn!(error = %e, "sync_log update failed"));
    }

    info!(
        sync_id        = %sync_id,
        symbol         = %symbol,
        interval       = %interval.as_str(),
        from_date      = %gap.from_date,
        to_date        = %gap.to_date,
        batches        = total_batches,
        inserted       = actual_inserted,
        skipped        = actual_skipped,
        failed_batches = total_failed,
        flush_ms       = flush_elapsed_ms,
        elapsed_ms     = gap_started.elapsed().as_millis() as u64,
        "Gap fetch complete (batched fallback)"
    );

    Ok((actual_inserted, actual_skipped, total_failed))
}

// ── run_manual_sync（對外入口）────────────────────────────────────────────────

pub async fn run_manual_sync(
    db_pool: PgPool,
    http_client: Client,
    rate_limiter: Arc<FinMindRateLimiter>,
    mut redis_conn: MultiplexedConnection,
    sync_id: String,
    symbols: Vec<String>,
    scope: SyncScope,
    buffer: Arc<tokio::sync::Mutex<BulkInsertBuffer>>,
    datasets: Vec<DatasetType>,
) {
    info!(sync_id = %sync_id, symbols = ?symbols, "Manual sync started");

    let mut buffer = buffer.lock().await;

    if let Err(e) = refresh_symbols_from_finmind(
        &db_pool,
        &http_client,
        &rate_limiter,
        SymbolFetchScope::MissingOnly(&symbols),
    )
    .await
    {
        warn!(
            error = %e,
            sync_id = %sync_id,
            "Failed to refresh symbols metadata before manual sync; continue with fallback"
        );
    }

    let trading_dates = match load_trading_dates_5y(&http_client, &rate_limiter, &mut redis_conn)
        .await
    {
        Ok(dates) => dates,
        Err(e) => {
            warn!(error = %e, sync_id = %sync_id, "Failed to load trading dates cache; fallback to empty set");
            HashSet::new()
        }
    };

    // 從 DB 確認哪些是 is_active = true
    let active_symbols = match fetch_active_symbols(&db_pool, Some(&symbols)).await {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "Failed to fetch active symbols");
            let _ = sync_log_update_status(
                &db_pool,
                &sync_id,
                SyncStatus::Failed.as_str(),
                Some(current_timestamp_ms()),
            )
            .await;
            return;
        }
    };

    let skipped: Vec<&String> = symbols
        .iter()
        .filter(|s| !active_symbols.contains(s))
        .collect();

    if !skipped.is_empty() {
        warn!(
            sync_id  = %sync_id,
            symbols  = ?skipped,
            count    = skipped.len(),
            "Delisted symbols excluded from sync"
        );
    }

    // 依使用者選擇的 datasets 建立同步器清單
    let syncer_list: Vec<Box<dyn DatasetSync>> = datasets
        .iter()
        .map(|d| -> Box<dyn DatasetSync> {
            match d {
                DatasetType::TaiwanStockPrice => Box::new(CandlesDataset::default()),
                DatasetType::TaiwanStockInstitutionalInvestorsBuySell => {
                    Box::new(InstitutionalInvestorsDataset)
                }
                DatasetType::TaiwanStockInfo => Box::new(StockInfoDataset),
                DatasetType::TaiwanStockTradingDate => Box::new(TradingDateDataset),
            }
        })
        .collect();

    let mut has_gap_error = false;

    // Step 2: 對每個缺口執行補資料
    // gaps 以 symbol 為單位分組（目前每個 symbol 只有一個 interval，所以一對一）
    for symbol in &active_symbols {
        let _ = update_symbol_progress(
            &mut redis_conn,
            &sync_id,
            symbol,
            SymbolSyncStatus::Running,
            None,
            None,
        )
        .await;

        let mut symbol_failed = false;
        let mut result_gap_a: Option<GapProgress> = None;
        let mut result_gap_b: Option<GapProgress> = None;

        for syncer in &syncer_list {
            let mut ctx = SyncContext {
                db_pool: &db_pool,
                http_client: &http_client,
                rate_limiter: &rate_limiter,
                redis: &mut redis_conn,
                buffer: &mut buffer,
                sync_id: &sync_id,
                trading_dates: &trading_dates,
            };

            let (gap_a, gap_b) = match syncer.detect_gaps(&scope, symbol, &ctx).await {
                Ok(gaps) => gaps,
                Err(e) => {
                    error!(error = %e, symbol = %symbol, dataset = %syncer.name(), "detect_gaps failed");
                    symbol_failed = true;
                    has_gap_error = true;
                    continue;
                }
            };

            for (gap_opt, is_gap_a) in [(&gap_a, true), (&gap_b, false)] {
                if let Some(gap) = gap_opt {
                    match syncer.fetch_and_insert(symbol, gap, &mut ctx).await {
                        Ok(r) => {
                            let progress = GapProgress {
                                from_ms: gap
                                    .from_date
                                    .and_hms_opt(0, 0, 0)
                                    .unwrap()
                                    .and_utc()
                                    .timestamp_millis(),
                                to_ms: gap
                                    .to_date
                                    .and_hms_opt(0, 0, 0)
                                    .unwrap()
                                    .and_utc()
                                    .timestamp_millis(),
                                inserted: r.inserted,
                                skipped: r.skipped,
                                failed: r.failed,
                                completed: r.failed == 0,
                            };
                            // 只保留最後一個有資料的 gap 結果回寫前端
                            if is_gap_a {
                                result_gap_a = Some(progress);
                            } else {
                                result_gap_b = Some(progress);
                            }
                        }
                        Err(e) => {
                            error!(error = %e, symbol = %symbol, dataset = %syncer.name());
                            symbol_failed = true;
                            has_gap_error = true;
                        }
                    }
                }
            }
        }

        // 所有 dataset 跑完才標最終狀態
        let symbol_status = if symbol_failed {
            SymbolSyncStatus::Failed
        } else {
            SymbolSyncStatus::Completed
        };
        let _ = update_symbol_progress(
            &mut redis_conn,
            &sync_id,
            symbol,
            symbol_status,
            result_gap_a,
            result_gap_b,
        )
        .await;
    }

    // Step 3: 依執行結果標記最終狀態
    let completed_at = current_timestamp_ms();
    let final_status = if has_gap_error {
        SyncStatus::Failed.as_str()
    } else {
        SyncStatus::Completed.as_str()
    };

    let _ = sync_log_update_status(&db_pool, &sync_id, final_status, Some(completed_at)).await;

    info!(
        sync_id       = %sync_id,
        final_status  = %final_status,
        has_gap_error = has_gap_error,
        completed_at  = completed_at,
        "Manual sync completed"
    );
}

// ── 工具函數 ──────────────────────────────────────────────────────────────────

pub(crate) async fn load_trading_dates_5y(
    http_client: &Client,
    rate_limiter: &Arc<FinMindRateLimiter>,
    redis: &mut MultiplexedConnection,
) -> Result<HashSet<NaiveDate>, BridgeError> {
    let cached: Option<String> = redis.get(REDIS_TRADING_DATES_KEY).await.ok();
    if let Some(raw) = cached {
        if let Ok(days) = serde_json::from_str::<Vec<String>>(&raw) {
            let parsed: HashSet<NaiveDate> = days
                .into_iter()
                .filter_map(|d| NaiveDate::parse_from_str(&d, FINMIND_DATE_FORMAT).ok())
                .collect();
            if !parsed.is_empty() {
                return Ok(parsed);
            }
        }
    }

    let today = Local::now().date_naive();

    // 1. 計算 5 年前的「那一天」（精確處理曆法與閏年）
    let five_years_ago_date = today
        .checked_sub_months(Months::new(5 * 12))
        .expect("日期計算超出範圍");

    // 2. 取得該日期的「年份」，並設定為該年的 1 月 1 日
    let start = NaiveDate::from_ymd_opt(five_years_ago_date.year(), 1, 1).expect("無效的起始日期");
    // 3. 取得「今年」的 12 月 31 日
    let end = NaiveDate::from_ymd_opt(today.year(), 12, 31).expect("無效的結束日期");

    let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();

    rate_limiter.acquire().await;
    let fetched = fetch_trading_dates(
        http_client,
        &api_token,
        &start.format(FINMIND_DATE_FORMAT).to_string(),
        &end.format(FINMIND_DATE_FORMAT).to_string(),
    )
    .await?;
    rate_limiter.mark_request_used().await;

    if !fetched.is_empty() {
        let mut vec_days: Vec<String> = fetched
            .iter()
            .map(|d| d.format(FINMIND_DATE_FORMAT).to_string())
            .collect();
        vec_days.sort();
        if let Ok(payload) = serde_json::to_string(&vec_days) {
            let _: redis::RedisResult<()> = redis
                .set_ex(
                    REDIS_TRADING_DATES_KEY,
                    payload,
                    REDIS_TRADING_DATES_TTL_SECS,
                )
                .await;
        }
    }

    Ok(fetched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_range_monthly_batches_single_month() {
        let range = DateRange {
            from_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            to_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        };
        let batches = range.into_monthly_batches();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].from_date.to_string(), "2026-01-01");
        assert_eq!(batches[0].to_date.to_string(), "2026-01-31");
    }

    #[test]
    fn test_date_range_monthly_batches_two_months() {
        let range = DateRange {
            from_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            to_date: NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
        };
        let batches = range.into_monthly_batches();
        assert_eq!(batches.len(), 2);
    }
}
