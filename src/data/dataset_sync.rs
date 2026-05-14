use async_trait::async_trait;
use chrono::{Local, Months, NaiveDate};
use redis::aio::MultiplexedConnection;
use reqwest::Client;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;

use crate::data::db::BulkInsertBuffer;
use crate::data::fetch_rate_limiter::FinMindRateLimiter;
use crate::data::manual_sync::{DateRange, SyncScope};
use crate::domain::BridgeError;
use crate::models::DateColumnType;

/// 單一 dataset 同步的結果
#[derive(Debug, Default)]
pub struct DatasetSyncResult {
    pub inserted: i32,
    pub skipped: i32,
    pub failed: i32,
}

/// 同步執行所需的共用依賴，避免每個函數傳一堆參數
pub struct SyncContext<'a> {
    pub db_pool: &'a PgPool,
    pub http_client: &'a Client,
    pub rate_limiter: &'a Arc<FinMindRateLimiter>,
    pub redis: &'a mut MultiplexedConnection,
    pub buffer: &'a mut BulkInsertBuffer,
    pub sync_id: &'a str,
    pub trading_dates: &'a HashSet<NaiveDate>,
}

/// 各 dataset 同步行為的抽象介面
#[async_trait]
pub trait DatasetSync: Send + Sync {
    /// dataset 的顯示名稱，用於 log
    fn name(&self) -> &str;

    /// 偵測資料缺口，預設實作直接回傳完整範圍（不查 DB）
    /// TaiwanStockPrice 和 InstitutionalInvestors 需 override
    async fn detect_gaps(
        &self,
        scope: &SyncScope,
        _symbol: &str,
        _ctx: &SyncContext<'_>,
    ) -> Result<(Option<DateRange>, Option<DateRange>), BridgeError> {
        let today = Local::now().date_naive();
        let from = scope.from_date.unwrap_or_else(|| {
            today
                .checked_sub_months(Months::new(5 * 12))
                .expect("日期計算超出範圍")
        });
        let to = scope.to_date.unwrap_or_else(|| today);
        Ok((
            Some(DateRange {
                from_date: from,
                to_date: to,
            }),
            None,
        ))
    }

    /// 抓取指定範圍資料並寫入 DB
    async fn fetch_and_insert(
        &self,
        symbol: &str,
        gap: &DateRange,
        ctx: &mut SyncContext<'_>,
    ) -> Result<DatasetSyncResult, BridgeError>;
}

/// 共用缺口偵測邏輯，供 CandlesDataset 和 InstitutionalDataset 呼叫
/// 查指定資料表的 MIN/MAX date，算出 Gap A（歷史段）和 Gap B（近期段）
pub async fn detect_gaps_by_date(
    db_pool: &PgPool,
    symbol: &str,
    table_name: &str,
    date_column: &str,
    column_type: DateColumnType,
    scope: &SyncScope,
    trading_dates: &HashSet<NaiveDate>,
    finmind_earliest: NaiveDate,
) -> Result<(Option<DateRange>, Option<DateRange>), BridgeError> {
    let today = chrono::Utc::now().date_naive();

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

    if range_start > range_end {
        return Ok((None, None));
    }

    // timestamp_ms 欄位直接轉 bigint；DATE 欄位需先轉 timestamp 再取 epoch
    let sql = match column_type {
        DateColumnType::TimestampMs => format!(
            // bigint 直接 MIN/MAX，不做任何轉換
            "SELECT MIN({col}) AS oldest_ms, MAX({col}) AS newest_ms \
             FROM {table} WHERE symbol = $1",
            col = date_column,
            table = table_name,
        ),
        DateColumnType::Date => format!(
            // DATE 型別需轉 epoch 毫秒
            "SELECT \
               EXTRACT(EPOCH FROM MIN({col}::timestamp))::bigint * 1000 AS oldest_ms, \
               EXTRACT(EPOCH FROM MAX({col}::timestamp))::bigint * 1000 AS newest_ms \
             FROM {table} WHERE symbol = $1",
            col = date_column,
            table = table_name,
        ),
    };

    let row: (Option<i64>, Option<i64>) = sqlx::query_as(&sql)
        .bind(symbol)
        .fetch_one(db_pool)
        .await
        .map_err(|e| {
            tracing::error!(
                error = %e,
                sql = %sql,
                symbol = %symbol,
                table = %table_name,
                column = %date_column,
                "detect_gaps_by_date query failed"
            );
            BridgeError::from_db("detect_gaps_by_date failed", e)
        })?;

    let gap_a_and_b = compute_gaps(row.0, row.1, range_start, range_end, trading_dates);
    Ok(gap_a_and_b)
}

/// MIN/MAX → (Gap A, Gap B) 純計算，方便單元測試
fn compute_gaps(
    oldest_ms: Option<i64>,
    newest_ms: Option<i64>,
    range_start: NaiveDate,
    range_end: NaiveDate,
    trading_dates: &HashSet<NaiveDate>,
) -> (Option<DateRange>, Option<DateRange>) {
    use chrono::Duration;

    match (oldest_ms, newest_ms) {
        (None, _) | (_, None) => {
            // DB 無資料，整段都是缺口
            (
                Some(DateRange {
                    from_date: range_start,
                    to_date: range_end,
                }),
                None,
            )
        }
        (Some(oldest_ms), Some(newest_ms)) => {
            let db_oldest = ms_to_naive_date(oldest_ms);
            let db_newest = ms_to_naive_date(newest_ms);

            let gap_a = if range_start < db_oldest {
                let to = (db_oldest - Duration::days(1)).min(range_end);
                if range_start <= to && has_trading_day(trading_dates, range_start, to) {
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
                if has_trading_day(trading_dates, from, range_end) {
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

            (gap_a, gap_b)
        }
    }
}

fn ms_to_naive_date(ms: i64) -> NaiveDate {
    let secs = ms / 1000;
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .date_naive()
}

fn has_trading_day(trading_dates: &HashSet<NaiveDate>, from: NaiveDate, to: NaiveDate) -> bool {
    let mut cursor = from;
    while cursor <= to {
        if trading_dates.contains(&cursor) {
            return true;
        }
        cursor += chrono::Duration::days(1);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::FINMIND_DATE_FORMAT;
    use chrono::NaiveDate;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, FINMIND_DATE_FORMAT).unwrap()
    }

    #[test]
    fn test_compute_gaps_no_data_returns_full_range() {
        let (a, b) = compute_gaps(
            None,
            None,
            date("2026-01-01"),
            date("2026-01-31"),
            &HashSet::new(),
        );
        assert!(a.is_some());
        assert!(b.is_none());
    }

    #[test]
    fn test_compute_gaps_only_gap_b() {
        let oldest = date("2026-01-01")
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let newest = date("2026-01-20")
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let mut trading = HashSet::new();
        trading.insert(date("2026-01-21"));
        let (a, b) = compute_gaps(
            Some(oldest),
            Some(newest),
            date("2026-01-01"),
            date("2026-01-31"),
            &trading,
        );
        assert!(a.is_none());
        assert!(b.is_some());
    }
}
