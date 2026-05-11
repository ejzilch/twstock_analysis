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
use crate::data::db::{sync_log_update_counts, sync_log_update_status, BulkInsertBuffer};
use crate::data::fetch::{fetch_range, fetch_trading_dates};
use crate::data::fetch_rate_limiter::{FinMindRateLimiter, SyncProgress};
use crate::data::implementations::{PostgresDbWriter, RedisInvalidator};
use crate::data::models::current_timestamp_ms;
use crate::data::symbol_sync::{
    fetch_active_symbols, get_finmind_earliest_ms, refresh_symbols_from_finmind,
};
use crate::domain::BridgeError;
use crate::models::enums::{Interval, SymbolFetchScope, SymbolSyncStatus, SyncStatus};
use crate::services::sync_state::{
    is_sync_cancel_requested, update_symbol_progress, update_sync_status,
};
use crate::services::sync_types::GapProgress;

use chrono::{Datelike, Duration, NaiveDate, Utc};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use reqwest::Client;
use sqlx::PgPool;
use std::{collections::HashSet, sync::Arc, time::Instant};
use tracing::{error, info, warn};

// ── 所有支援的 K 線粒度 ────────────────────────────────────────────────────────

const ALL_INTERVALS: &[Interval] = &[Interval::OneDay];

#[derive(Debug, Clone)]
pub struct SyncScope {
    pub full_sync: bool,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub intervals: Vec<Interval>,
}

// ── 缺口定義 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GapInfo {
    pub symbol: String,
    pub interval: Interval,
    pub gap_a: Option<DateRange>,
    pub gap_b: Option<DateRange>,
    pub gap_internal: Vec<DateRange>,
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

// ── detect_gaps ───────────────────────────────────────────────────────────────

pub async fn detect_gaps(
    db_pool: &PgPool,
    symbols: &[String],
    scope: &SyncScope,
    trading_dates: &HashSet<NaiveDate>,
) -> Result<Vec<GapInfo>, BridgeError> {
    let today = Utc::now().date_naive();
    let selected_intervals = if scope.intervals.is_empty() {
        ALL_INTERVALS.to_vec()
    } else {
        scope.intervals.clone()
    };
    let mut gaps = Vec::new();

    for symbol in symbols {
        let finmind_earliest_ms = get_finmind_earliest_ms(db_pool, symbol).await?;

        let finmind_earliest = finmind_earliest_ms
            .map(ms_to_naive_date)
            .unwrap_or_else(|| {
                let fallback = today - Duration::days(365 * 13);
                warn!(
                    symbol = %symbol,
                    fallback_from = %fallback,
                    "finmind_earliest_ms is NULL, using fallback range for manual sync"
                );
                fallback
            });

        for interval in &selected_intervals {
            let gap_info = detect_gap_for_interval(
                db_pool,
                symbol,
                *interval,
                finmind_earliest,
                today,
                scope,
                trading_dates,
            )
            .await?;

            gaps.push(gap_info);
        }
    }

    Ok(gaps)
}

async fn detect_gap_for_interval(
    db_pool: &PgPool,
    symbol: &str,
    interval: Interval,
    finmind_earliest: NaiveDate,
    today: NaiveDate,
    scope: &SyncScope,
    trading_dates: &HashSet<NaiveDate>,
) -> Result<GapInfo, BridgeError> {
    let row = sqlx::query!(
        r#"
        SELECT
            MIN(timestamp_ms) AS db_oldest_ms,
            MAX(timestamp_ms) AS db_newest_ms
        FROM candles
        WHERE symbol = $1 AND interval = $2
        "#,
        symbol,
        interval.as_str(),
    )
    .fetch_one(db_pool)
    .await
    .map_err(|e| {
        error!(error = %e, symbol = %symbol, "Failed to query candle range");
        BridgeError::DatabaseError {
            context: "Failed to query candle range: ".into(),
            source: Some(Box::new(e)),
        }
    })?;

    let range_start = if scope.full_sync {
        finmind_earliest
    } else {
        scope
            .from_date
            .unwrap_or(finmind_earliest)
            .max(finmind_earliest)
    };
    let range_end = if scope.full_sync {
        today
    } else {
        scope.to_date.unwrap_or(today).min(today)
    };

    tracing::info!(range_start = %range_start, range_end = %range_end, "Check Gap");

    if range_start > range_end {
        return Ok(GapInfo {
            symbol: symbol.to_string(),
            interval,
            gap_a: None,
            gap_b: None,
            gap_internal: Vec::new(),
        });
    }

    let (gap_a, gap_b, gap_internal) = match (row.db_oldest_ms, row.db_newest_ms) {
        (None, _) | (_, None) => {
            info!(
                symbol = %symbol,
                interval = %interval.as_str(),
                "No data in DB, will fetch full range"
            );
            let gap_a = DateRange {
                from_date: range_start,
                to_date: range_end,
            };
            (Some(gap_a), None, Vec::new())
        }

        (Some(oldest_ms), Some(newest_ms)) => {
            let db_oldest = ms_to_naive_date(oldest_ms);
            let db_newest = ms_to_naive_date(newest_ms);

            let gap_a = if range_start < db_oldest {
                let to = (db_oldest - Duration::days(1)).min(range_end);
                if range_start <= to && has_trading_day_in_range(trading_dates, range_start, to) {
                    Some(DateRange {
                        from_date: range_start,
                        to_date: to,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let gap_b = if db_newest < range_end {
                let from = (db_newest + Duration::days(1)).max(range_start);
                if has_trading_day_in_range(trading_dates, from, range_end) {
                    Some(DateRange {
                        from_date: from,
                        to_date: range_end,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let internal_gaps = detect_internal_gaps(
                db_pool,
                symbol,
                interval,
                range_start.max(db_oldest),
                range_end.min(db_newest),
                trading_dates,
            )
            .await?;

            (gap_a, gap_b, internal_gaps)
        }
    };

    Ok(GapInfo {
        symbol: symbol.to_string(),
        interval,
        gap_a,
        gap_b,
        gap_internal,
    })
}

async fn detect_internal_gaps(
    db_pool: &PgPool,
    symbol: &str,
    interval: Interval,
    range_start: NaiveDate,
    range_end: NaiveDate,
    trading_dates: &HashSet<NaiveDate>,
) -> Result<Vec<DateRange>, BridgeError> {
    let rows = sqlx::query!(
        r#"
        WITH existing_days AS (
            SELECT DISTINCT to_timestamp(timestamp_ms / 1000)::date AS day
            FROM candles
            WHERE symbol = $1
              AND interval = $2
              AND to_timestamp(timestamp_ms / 1000)::date BETWEEN $3 AND $4
        ), sorted_days AS (
            SELECT day, LAG(day) OVER (ORDER BY day) AS prev_day
            FROM existing_days
        )
        SELECT
            (prev_day + interval '1 day')::date AS missing_from,
            (day - interval '1 day')::date AS missing_to
        FROM sorted_days
        WHERE prev_day IS NOT NULL
          AND day > prev_day + interval '1 day'
        ORDER BY missing_from
        "#,
        symbol,
        interval.as_str(),
        range_start,
        range_end,
    )
    .fetch_all(db_pool)
    .await
    .map_err(|e| BridgeError::DatabaseError {
        context: "Failed to detect internal missing ranges: ".into(),
        source: Some(Box::new(e)),
    })?;

    Ok(rows
        .into_iter()
        .filter_map(|row| match (row.missing_from, row.missing_to) {
            (Some(from_date), Some(to_date)) if from_date <= to_date => {
                Some(DateRange { from_date, to_date })
            }
            _ => None,
        })
        .filter_map(|r| {
            if has_trading_day_in_range(trading_dates, r.from_date, r.to_date) {
                Some(r)
            } else {
                None
            }
        })
        .collect())
}

fn has_trading_day_in_range(
    trading_dates: &HashSet<NaiveDate>,
    from_date: NaiveDate,
    to_date: NaiveDate,
) -> bool {
    let mut cursor = from_date;
    while cursor <= to_date {
        if trading_dates.contains(&cursor) {
            return true;
        }
        cursor += Duration::days(1);
    }
    false
}

// ── fetch_and_insert_gap ──────────────────────────────────────────────────────

// ── fetch_and_insert_gap ──────────────────────────────────────────────────────

pub async fn fetch_and_insert_gap(
    db_pool: &PgPool,
    http_client: &Client,
    rate_limiter: &Arc<FinMindRateLimiter>,
    buffer: &mut BulkInsertBuffer,
    redis: &mut MultiplexedConnection,
    sync_id: &str,
    gap_info: &GapInfo,
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
        symbol    = %gap_info.symbol,
        interval  = %gap_info.interval.as_str(),
        from_date = %gap.from_date,
        to_date   = %gap.to_date,
        "Starting gap fetch (single-request mode)"
    );

    // ── 取消檢查 ──────────────────────────────────────────────────────────────
    if is_sync_cancel_requested(redis, sync_id)
        .await
        .unwrap_or(false)
    {
        warn!(sync_id = %sync_id, symbol = %gap_info.symbol, "Sync cancel requested, stopping");
        return Err(BridgeError::internal("SYNC_CANCELLED"));
    }

    // ── 記錄進度 ──────────────────────────────────────────────────────────────
    rate_limiter
        .record_progress(SyncProgress {
            current_symbol: gap_info.symbol.clone(),
            current_interval: gap_info.interval.as_str().to_string(),
            current_date: gap.from_date.to_string(),
        })
        .await;

    // ── Rate limit 狀態更新 ───────────────────────────────────────────────────
    if let Some(resume_at_ms) = rate_limiter.predicted_resume_at_ms().await {
        let _ = update_sync_status(redis, sync_id, SyncStatus::RateLimitWaiting).await;
        info!(
            sync_id      = %sync_id,
            symbol       = %gap_info.symbol,
            resume_at_ms = resume_at_ms,
            "Rate limit reached; waiting for next window"
        );
    }

    rate_limiter.acquire().await.ok();
    let _ = update_sync_status(redis, sync_id, SyncStatus::Running).await;

    // ── 第一次嘗試：單次請求完整範圍 ─────────────────────────────────────────
    let from_str = gap.from_date.to_string();
    let to_str = gap.to_date.to_string();

    let candles = match fetch_range(
        http_client,
        &gap_info.symbol,
        gap_info.interval,
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
                symbol   = %gap_info.symbol,
                interval = %gap_info.interval.as_str(),
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
                symbol   = %gap_info.symbol,
                interval = %gap_info.interval.as_str(),
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
            symbol   = %gap_info.symbol,
            interval = %gap_info.interval.as_str(),
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
            gap_info,
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
        symbol       = %gap_info.symbol,
        interval     = %gap_info.interval.as_str(),
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

async fn fetch_and_insert_gap_batched(
    db_pool: &PgPool,
    http_client: &Client,
    rate_limiter: &Arc<FinMindRateLimiter>,
    buffer: &mut BulkInsertBuffer,
    redis: &mut MultiplexedConnection,
    sync_id: &str,
    gap_info: &GapInfo,
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
        symbol    = %gap_info.symbol,
        interval  = %gap_info.interval.as_str(),
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
            warn!(sync_id = %sync_id, symbol = %gap_info.symbol, "Sync cancel requested, stopping");
            return Err(BridgeError::internal("SYNC_CANCELLED"));
        }

        rate_limiter
            .record_progress(SyncProgress {
                current_symbol: gap_info.symbol.clone(),
                current_interval: gap_info.interval.as_str().to_string(),
                current_date: batch.from_date.to_string(),
            })
            .await;

        if let Some(resume_at_ms) = rate_limiter.predicted_resume_at_ms().await {
            let _ = update_sync_status(redis, sync_id, SyncStatus::RateLimitWaiting).await;
            info!(
                sync_id      = %sync_id,
                symbol       = %gap_info.symbol,
                resume_at_ms = resume_at_ms,
                "Rate limit reached; waiting for next window"
            );
        }

        rate_limiter.acquire().await.ok();
        let _ = update_sync_status(redis, sync_id, SyncStatus::Running).await;

        let from_str = batch.from_date.to_string();
        let to_str = batch.to_date.to_string();

        match fetch_range(
            http_client,
            &gap_info.symbol,
            gap_info.interval,
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
                    symbol     = %gap_info.symbol,
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
                    symbol   = %gap_info.symbol,
                    interval = %gap_info.interval.as_str(),
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
        symbol         = %gap_info.symbol,
        interval       = %gap_info.interval.as_str(),
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

    // Step 1: 偵測所有缺口
    let gaps = match detect_gaps(&db_pool, &active_symbols, &scope, &trading_dates).await {
        Ok(g) => g,
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "detect_gaps failed");
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

    let mut has_gap_error = false;
    let mut total_failed_batches = 0i32;

    // Step 2: 對每個缺口執行補資料
    // gaps 以 symbol 為單位分組（目前每個 symbol 只有一個 interval，所以一對一）
    for gap_info in &gaps {
        // ── symbol 開始：標記為 running ──────────────────────────────────────
        let _ = update_symbol_progress(
            &mut redis_conn,
            &sync_id,
            &gap_info.symbol,
            SymbolSyncStatus::Running,
            None,
            None,
        )
        .await;

        let mut symbol_failed = false;
        let mut result_gap_a: Option<GapProgress> = None;
        let mut result_gap_b: Option<GapProgress> = None;

        // ── gap_a ─────────────────────────────────────────────────────────────
        if let Some(ref gap_a) = gap_info.gap_a {
            match fetch_and_insert_gap(
                &db_pool,
                &http_client,
                &rate_limiter,
                &mut buffer,
                &mut redis_conn,
                &sync_id,
                gap_info,
                gap_a,
            )
            .await
            {
                Ok((inserted, skipped, failed)) => {
                    total_failed_batches += failed;
                    result_gap_a = Some(GapProgress {
                        from_ms: gap_a
                            .from_date
                            .and_hms_opt(0, 0, 0)
                            .unwrap()
                            .and_utc()
                            .timestamp_millis(),
                        to_ms: gap_a
                            .to_date
                            .and_hms_opt(0, 0, 0)
                            .unwrap()
                            .and_utc()
                            .timestamp_millis(),
                        inserted,
                        skipped,
                        failed,
                        completed: failed == 0,
                    });
                }
                Err(e) => {
                    has_gap_error = true;
                    symbol_failed = true;
                    error!(
                        error   = %e,
                        sync_id = %sync_id,
                        symbol  = %gap_info.symbol,
                        "Gap A fetch failed"
                    );
                }
            }
        }

        // ── gap_b ─────────────────────────────────────────────────────────────
        if let Some(ref gap_b) = gap_info.gap_b {
            match fetch_and_insert_gap(
                &db_pool,
                &http_client,
                &rate_limiter,
                &mut buffer,
                &mut redis_conn,
                &sync_id,
                gap_info,
                gap_b,
            )
            .await
            {
                Ok((inserted, skipped, failed)) => {
                    total_failed_batches += failed;
                    result_gap_b = Some(GapProgress {
                        from_ms: gap_b
                            .from_date
                            .and_hms_opt(0, 0, 0)
                            .unwrap()
                            .and_utc()
                            .timestamp_millis(),
                        to_ms: gap_b
                            .to_date
                            .and_hms_opt(0, 0, 0)
                            .unwrap()
                            .and_utc()
                            .timestamp_millis(),
                        inserted,
                        skipped,
                        failed,
                        completed: failed == 0,
                    });
                }
                Err(e) => {
                    has_gap_error = true;
                    symbol_failed = true;
                    error!(
                        error   = %e,
                        sync_id = %sync_id,
                        symbol  = %gap_info.symbol,
                        "Gap B fetch failed"
                    );
                }
            }
        }

        // ── gap_internal（不回寫個別進度，只累計失敗數）──────────────────────
        for gap_internal in &gap_info.gap_internal {
            match fetch_and_insert_gap(
                &db_pool,
                &http_client,
                &rate_limiter,
                &mut buffer,
                &mut redis_conn,
                &sync_id,
                gap_info,
                gap_internal,
            )
            .await
            {
                Ok((_, _, failed)) => total_failed_batches += failed,
                Err(e) => {
                    has_gap_error = true;
                    symbol_failed = true;
                    error!(
                        error   = %e,
                        sync_id = %sync_id,
                        symbol  = %gap_info.symbol,
                        "Internal gap fetch failed"
                    );
                }
            }
        }

        // ── symbol 完成：回寫最終狀態到 Redis ────────────────────────────────
        let symbol_status = if symbol_failed {
            SymbolSyncStatus::Failed
        } else {
            SymbolSyncStatus::Completed
        };

        let _ = update_symbol_progress(
            &mut redis_conn,
            &sync_id,
            &gap_info.symbol,
            symbol_status,
            result_gap_a,
            result_gap_b,
        )
        .await;
    }

    // Step 3: 依執行結果標記最終狀態
    let completed_at = current_timestamp_ms();
    let final_status = if has_gap_error || total_failed_batches > 0 {
        SyncStatus::Failed.as_str()
    } else {
        SyncStatus::Completed.as_str()
    };

    let _ = sync_log_update_status(&db_pool, &sync_id, final_status, Some(completed_at)).await;

    info!(
        sync_id      = %sync_id,
        final_status = %final_status,
        failed       = total_failed_batches,
        completed_at = completed_at,
        "Manual sync completed"
    );
}

// ── 工具函數 ──────────────────────────────────────────────────────────────────

fn ms_to_naive_date(ms: i64) -> NaiveDate {
    let secs = ms / 1000;
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .date_naive()
}

async fn load_trading_dates_5y(
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

    let today = Utc::now().date_naive();
    let start_year = today.year() - 5;
    let start =
        NaiveDate::from_ymd_opt(start_year, 1, 1).unwrap_or(today - Duration::days(365 * 5));
    let end = NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap_or(today);
    let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();

    rate_limiter.acquire().await.ok();
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
    use chrono::Datelike;

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

    #[test]
    fn test_ms_to_naive_date() {
        let date = ms_to_naive_date(1735689600000);
        assert_eq!(date.year(), 2026);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 1);
    }
}
