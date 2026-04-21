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
use crate::constants::{FINMIND_API_TOKEN_ENV, MANUAL_SYNC_BATCH_DAYS};
use crate::core::BridgeError;
use crate::data::db::{sync_log_update_counts, sync_log_update_status, BulkInsertBuffer};
use crate::data::fetch::fetch_range;
use crate::data::fetch_rate_limiter::{FinMindRateLimiter, SyncProgress};

use crate::data::implementations::{PostgresDbWriter, RedisInvalidator};
use crate::data::models::current_timestamp_ms;
use crate::data::symbol_sync::get_finmind_earliest_ms;
use crate::models::enums::Interval;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use redis::aio::MultiplexedConnection;
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info, warn};

// ── 所有支援的 K 線粒度（手動同步全部補齊）────────────────────────────────────

const ALL_INTERVALS: &[Interval] = &[
    Interval::OneMin,
    Interval::FiveMin,
    Interval::FifteenMin,
    Interval::OneHour,
    Interval::FourHours,
    Interval::OneDay,
];

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
///
/// 對每一檔股票、每一個粒度執行 SELECT MIN/MAX，
/// 與 finmind_earliest_ms 比對，計算兩段缺口。
///
/// 不打 FinMind API，純 DB 查詢。
pub async fn detect_gaps(
    db_pool: &PgPool,
    symbols: &[String],
) -> Result<Vec<GapInfo>, BridgeError> {
    let today = Utc::now().date_naive();
    let mut gaps = Vec::new();

    for symbol in symbols {
        // 查詢 FinMind 最早可提供日期
        let finmind_earliest_ms = get_finmind_earliest_ms(db_pool, symbol).await?;

        let finmind_earliest = match finmind_earliest_ms {
            Some(ms) => ms_to_naive_date(ms),
            None => {
                warn!(symbol = %symbol, "finmind_earliest_ms is NULL, skipping gap detection");
                continue;
            }
        };

        for &interval in ALL_INTERVALS {
            let gap_info =
                detect_gap_for_interval(db_pool, symbol, interval, finmind_earliest, today).await?;

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
) -> Result<GapInfo, BridgeError> {
    // 查詢 DB 現有資料的頭尾
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

    let (gap_a, gap_b) = match (row.db_oldest_ms, row.db_newest_ms) {
        // DB 完全無資料 → 全段補
        (None, _) | (_, None) => {
            info!(
                symbol = %symbol,
                interval = %interval.as_str(),
                "No data in DB, will fetch full range"
            );
            let gap_a = DateRange {
                from_date: finmind_earliest,
                to_date: today,
            };
            (Some(gap_a), None)
        }

        (Some(oldest_ms), Some(newest_ms)) => {
            let db_oldest = ms_to_naive_date(oldest_ms);
            let db_newest = ms_to_naive_date(newest_ms);

            // 缺口 A（歷史段）：FinMind最早 ~ DB最舊 - 1天
            let gap_a = if finmind_earliest < db_oldest {
                let to = db_oldest - Duration::days(1);
                if finmind_earliest <= to {
                    Some(DateRange {
                        from_date: finmind_earliest,
                        to_date: to,
                    })
                } else {
                    None
                }
            } else {
                None // 歷史段已完整
            };

            // 缺口 B（近期段）：DB最新 + 1天 ~ 今天
            let gap_b = if db_newest < today {
                let from = db_newest + Duration::days(1);
                Some(DateRange {
                    from_date: from,
                    to_date: today,
                })
            } else {
                None // 近期段已是今天
            };

            (gap_a, gap_b)
        }
    };

    Ok(GapInfo {
        symbol: symbol.to_string(),
        interval,
        gap_a,
        gap_b,
    })
}

// ── fetch_and_insert_gap ──────────────────────────────────────────────────────

/// 對單一缺口分批請求 FinMind 並寫入 DB。
///
/// 每批請求 MANUAL_SYNC_BATCH_DAYS 天的資料（預設 30 天）。
/// 每次請求前呼叫 rate_limiter.acquire()，達上限時自動等待。
///
/// # Returns
/// (inserted, skipped, failed) 三個計數器。
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
    let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();

    let batches = gap.clone().into_monthly_batches();
    let total_batches = batches.len();
    let mut total_inserted = 0i32;
    let mut total_skipped = 0i32;
    let mut total_failed = 0i32;

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
        // 記錄進度（rate limit 等待後從此繼續）
        rate_limiter
            .record_progress(SyncProgress {
                current_symbol: gap_info.symbol.clone(),
                current_interval: gap_info.interval.as_str().to_string(),
                current_date: batch.from_date.to_string(),
            })
            .await;

        // 等待 rate limit（達上限時會在此 async 等待 1 小時）
        rate_limiter.acquire().await.ok();

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

                // INSERT ON CONFLICT DO NOTHING — 無法直接得知跳過數
                // 以 fetched 為近似值（實際跳過數由 DB 決定）
                total_inserted += fetched;

                // 定期更新 sync_log（每 10 批更新一次，減少 DB 寫入）
                if i % 10 == 0 {
                    sync_log_update_counts(db_pool, sync_id, fetched, 0, 0)
                        .await
                        .unwrap_or_else(|e| warn!(error = %e, "sync_log update failed"));
                }

                info!(
                    sync_id  = %sync_id,
                    symbol   = %gap_info.symbol,
                    batch    = format!("{}/{}", i + 1, total_batches),
                    fetched  = fetched,
                    "Batch complete"
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

    // 最終 flush buffer
    buffer.flush(&writer, &mut invalidator).await?;

    Ok((total_inserted, total_skipped, total_failed))
}

// ── run_manual_sync（對外入口）────────────────────────────────────────────────

/// 手動同步主流程，由 API handler 在背景 task 中呼叫。
///
/// 流程：
///   1. detect_gaps() 計算所有缺口
///   2. 對每個缺口呼叫 fetch_and_insert_gap()
///   3. 更新 sync_log status = 'completed'
pub async fn run_manual_sync(
    db_pool: PgPool,
    http_client: Client,
    rate_limiter: Arc<FinMindRateLimiter>,
    sync_id: String,
    symbols: Vec<String>,
) {
    info!(sync_id = %sync_id, symbols = ?symbols, "Manual sync started");

    // 建立共用的 BulkInsertBuffer 和 Redis 連線
    let mut buffer = BulkInsertBuffer::new();
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1".into());
    let redis_client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "Failed to connect to Redis");
            let _ =
                sync_log_update_status(&db_pool, &sync_id, "failed", Some(current_timestamp_ms()))
                    .await;
            return;
        }
    };
    let mut redis_conn = match redis_client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "Failed to get Redis connection");
            let _ =
                sync_log_update_status(&db_pool, &sync_id, "failed", Some(current_timestamp_ms()))
                    .await;
            return;
        }
    };

    // Step 1: 偵測所有缺口
    let gaps = match detect_gaps(&db_pool, &symbols).await {
        Ok(g) => g,
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "detect_gaps failed");
            let _ =
                sync_log_update_status(&db_pool, &sync_id, "failed", Some(current_timestamp_ms()))
                    .await;
            return;
        }
    };

    // Step 2: 對每個缺口執行補資料
    for gap_info in &gaps {
        // 處理缺口 A（歷史段）
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

            if let Err(e) = result {
                error!(
                    error   = %e,
                    sync_id = %sync_id,
                    symbol  = %gap_info.symbol,
                    "Gap A fetch failed"
                );
            }
        }

        // 處理缺口 B（近期段）
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

            if let Err(e) = result {
                error!(
                    error   = %e,
                    sync_id = %sync_id,
                    symbol  = %gap_info.symbol,
                    "Gap B fetch failed"
                );
            }
        }
    }

    // Step 3: 標記完成
    let completed_at = current_timestamp_ms();
    let _ = sync_log_update_status(&db_pool, &sync_id, "completed", Some(completed_at)).await;

    info!(
        sync_id      = %sync_id,
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

    #[test]
    fn test_ms_to_naive_date() {
        // 2026-01-01 00:00:00 UTC = 1735689600000 ms
        let date = ms_to_naive_date(1735689600000);
        assert_eq!(date.year(), 2026);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 1);
    }
}
