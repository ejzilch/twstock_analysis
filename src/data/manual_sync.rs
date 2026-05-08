/// 手動補資料核心邏輯。
///
/// 流程：
///   1. detect_gaps()    — 查 DB MIN/MAX，計算缺口 A（歷史段）和缺口 B（近期段）
///   2. fetch_and_insert_gap() — 對缺口分批請求 FinMind，INSERT ON CONFLICT DO NOTHING
///   3. RateLimitQueue   — 由 FinMindRateLimiter.acquire() 管理，達上限自動等待
///
/// 設計原則：
///   - 先查 DB 確認缺口，再打 FinMind API，不浪費請求配額
///   - 每批請求一個月的資料，控制單次請求大小
///   - 排程與手動同步共用 FinMindRateLimiter 實例
///   - Redis 連線由呼叫端（AppState）統一管理，不在此自行建立
use crate::constants::{
    FINMIND_API_TOKEN_ENV, MANUAL_SYNC_BATCH_DAYS, REDIS_TRADING_DATES_KEY,
    REDIS_TRADING_DATES_TTL_SECS,
};
use crate::data::db::{sync_log_update_counts, sync_log_update_status, BulkInsertBuffer};
use crate::data::fetch::{fetch_range, fetch_stock_info_map, fetch_trading_dates};
use crate::data::fetch_rate_limiter::{FinMindRateLimiter, SyncProgress};
use crate::data::implementations::{PostgresDbWriter, RedisInvalidator};
use crate::data::models::current_timestamp_ms;
use crate::data::symbol_sync::{get_finmind_earliest_ms, upsert_symbols, SymbolSyncData};
use crate::domain::BridgeError;
use crate::models::enums::{DataSource, Exchange, Interval, SyncStatus};
use crate::services::sync_state::{is_sync_cancel_requested, update_sync_status};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use reqwest::Client;
use sqlx::PgPool;
use std::{collections::HashSet, sync::Arc, time::Instant};
use tracing::{error, info, warn};

// ── 所有支援的 K 線粒度（手動同步全部補齊）────────────────────────────────────

const ALL_INTERVALS: &[Interval] = &[Interval::OneDay];

#[derive(Debug, Clone)]
pub struct SyncScope {
    pub full_sync: bool,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub intervals: Vec<Interval>,
}

// ── 缺口定義 ──────────────────────────────────────────────────────────────────

/// 單一股票、單一粒度的缺口資訊。
#[derive(Debug, Clone)]
pub struct GapInfo {
    pub symbol: String,
    pub interval: Interval,
    /// 缺口 A（歷史段）：FinMind 最早 ~ DB 最舊 - 1 天
    pub gap_a: Option<DateRange>,
    /// 缺口 B（近期段）：DB 最新 + 1 天 ~ 今天
    pub gap_b: Option<DateRange>,
    /// 缺口 C（區間內部）：DB 內部斷點造成的缺值（精準到連續區段）
    pub gap_internal: Vec<DateRange>,
}

/// 日期範圍（含頭含尾）。
#[derive(Debug, Clone)]
pub struct DateRange {
    pub from_date: NaiveDate,
    pub to_date: NaiveDate,
}

impl DateRange {
    /// 依 MANUAL_SYNC_BATCH_DAYS 切分為多個批次。
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

/// 查詢 DB，計算指定股票清單的所有缺口。
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

/// 針對單一股票 + 粒度計算缺口。
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
                if range_start <= to {
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
                Some(DateRange {
                    from_date: from,
                    to_date: range_end,
                })
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

/// 對單一缺口分批請求 FinMind 並寫入 DB。
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
    // 問題查找
    let gap_started = Instant::now();

    let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();

    let batches = gap.clone().into_monthly_batches();
    let total_batches = batches.len();
    let mut total_failed = 0i32;

    let initial_inserted = buffer.total_inserted();
    let initial_skipped = buffer.total_skipped();

    info!(
        sync_id    = %sync_id,
        symbol     = %gap_info.symbol,
        interval   = %gap_info.interval.as_str(),
        from_date  = %gap.from_date,
        to_date    = %gap.to_date,
        batches    = total_batches,
        "Starting gap fetch"
    );

    let writer = PostgresDbWriter::new(db_pool.clone());
    let mut invalidator = RedisInvalidator::new(redis.clone());

    for (i, batch) in batches.iter().enumerate() {
        // 問題查找
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
                sync_id = %sync_id,
                symbol = %gap_info.symbol,
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
            &api_token,
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
                            error!(
                                error = %e,
                                sync_id = %sync_id,
                                "Failed to push candle to buffer"
                            );
                            e
                        })?;
                }

                info!(
                    sync_id  = %sync_id,
                    symbol   = %gap_info.symbol,
                    batch    = format!("{}/{}", i + 1, total_batches),
                    fetched  = fetched,
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
        sync_id = %sync_id,
        symbol = %gap_info.symbol,
        interval = %gap_info.interval.as_str(),
        from_date = %gap.from_date,
        to_date = %gap.to_date,
        batches = total_batches,
        inserted = actual_inserted,
        skipped = actual_skipped,
        failed_batches = total_failed,
        flush_ms = flush_elapsed_ms,
        total_elapsed_ms = gap_started.elapsed().as_millis() as u64,
        db_pool_size = db_pool.size(),
        db_pool_idle = db_pool.num_idle(),
        "Gap execution diagnostics"
    );

    Ok((actual_inserted, actual_skipped, total_failed))
}

// ── run_manual_sync（對外入口）────────────────────────────────────────────────

/// 手動同步主流程，由 admin_sync service 在背景 task 中呼叫。
///
/// Redis 連線由呼叫端從 AppState 取得後傳入，不在此自行建立。
pub async fn run_manual_sync(
    db_pool: PgPool,
    http_client: Client,
    rate_limiter: Arc<FinMindRateLimiter>,
    mut redis_conn: MultiplexedConnection, // ← 由呼叫端傳入，不再自己 open
    sync_id: String,
    symbols: Vec<String>,
    scope: SyncScope,
    buffer: Arc<tokio::sync::Mutex<BulkInsertBuffer>>,
) {
    info!(sync_id = %sync_id, symbols = ?symbols, "Manual sync started");

    let mut buffer = buffer.lock().await;

    if let Err(e) =
        ensure_symbols_metadata(&db_pool, &http_client, &rate_limiter, &symbols, &sync_id).await
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

    // Step 1: 偵測所有缺口
    let gaps = match detect_gaps(&db_pool, &symbols, &scope, &trading_dates).await {
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
    for gap_info in &gaps {
        if let Some(ref gap_a) = gap_info.gap_a {
            let result = fetch_and_insert_gap(
                &db_pool,
                &http_client,
                &rate_limiter,
                &mut buffer,
                &mut redis_conn,
                &sync_id,
                gap_info,
                gap_a,
            )
            .await;

            match result {
                Ok((_, _, failed)) => total_failed_batches += failed,
                Err(e) => {
                    has_gap_error = true;
                    error!(
                        error   = %e,
                        sync_id = %sync_id,
                        symbol  = %gap_info.symbol,
                        "Gap A fetch failed"
                    );
                }
            }
        }

        if let Some(ref gap_b) = gap_info.gap_b {
            let result = fetch_and_insert_gap(
                &db_pool,
                &http_client,
                &rate_limiter,
                &mut buffer,
                &mut redis_conn,
                &sync_id,
                gap_info,
                gap_b,
            )
            .await;

            match result {
                Ok((_, _, failed)) => total_failed_batches += failed,
                Err(e) => {
                    has_gap_error = true;
                    error!(
                        error   = %e,
                        sync_id = %sync_id,
                        symbol  = %gap_info.symbol,
                        "Gap B fetch failed"
                    );
                }
            }
        }

        for gap_internal in &gap_info.gap_internal {
            let result = fetch_and_insert_gap(
                &db_pool,
                &http_client,
                &rate_limiter,
                &mut buffer,
                &mut redis_conn,
                &sync_id,
                gap_info,
                gap_internal,
            )
            .await;

            match result {
                Ok((_, _, failed)) => total_failed_batches += failed,
                Err(e) => {
                    has_gap_error = true;
                    error!(
                        error   = %e,
                        sync_id = %sync_id,
                        symbol  = %gap_info.symbol,
                        "Internal gap fetch failed"
                    );
                }
            }
        }
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

async fn ensure_symbols_metadata(
    db_pool: &PgPool,
    http_client: &Client,
    rate_limiter: &Arc<FinMindRateLimiter>,
    symbols: &[String],
    sync_id: &str,
) -> Result<(), BridgeError> {
    if symbols.is_empty() {
        return Ok(());
    }

    let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();

    rate_limiter.acquire().await.ok();
    let stock_info_map = fetch_stock_info_map(http_client, &api_token).await?;
    rate_limiter.mark_request_used().await;

    let now_ms = current_timestamp_ms();
    let upsert_payload: Vec<SymbolSyncData> = symbols
        .iter()
        .map(|symbol| {
            let info = stock_info_map.get(symbol);
            SymbolSyncData {
                symbol: symbol.clone(),
                name: info
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| format!("股票 {}", symbol)),
                exchange: info.map(|s| s.exchange).unwrap_or(Exchange::Twse),
                data_source: DataSource::FinMind,
                finmind_earliest_ms: None,
                latest_ms: now_ms,
                is_active: true,
            }
        })
        .collect();

    let summary = upsert_symbols(db_pool, &upsert_payload, now_ms).await?;
    info!(
        sync_id = %sync_id,
        inserted = summary.inserted,
        updated = summary.updated,
        failed = summary.failed,
        "Manual sync metadata upsert completed"
    );

    Ok(())
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
                .filter_map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
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
        &start.format("%Y-%m-%d").to_string(),
        &end.format("%Y-%m-%d").to_string(),
    )
    .await?;
    rate_limiter.mark_request_used().await;

    if !fetched.is_empty() {
        let mut vec_days: Vec<String> = fetched
            .iter()
            .map(|d| d.format("%Y-%m-%d").to_string())
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
