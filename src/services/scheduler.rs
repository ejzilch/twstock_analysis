use crate::app_state::AppState;
use crate::constants::FINMIND_DATE_FORMAT;
use crate::data::dataset_sync::{DatasetSync, SyncContext};
use crate::data::datasets::{
    candles::CandlesDataset, institutional_investors::InstitutionalInvestorsDataset,
    stock_info::StockInfoDataset, trading_date::TradingDateDataset,
};
use crate::data::db::{
    sync_log_create, sync_log_update_counts, sync_log_update_status, SyncLogEntry,
};
use crate::data::manual_sync::{load_trading_dates_5y, DateRange, SyncScope};
use crate::data::models::current_timestamp_ms;
use crate::data::symbol_sync::fetch_active_symbols;
use crate::models::enums::{SymbolSyncStatus, SyncStatus};
use crate::services::sync_state::{save_sync_state, update_symbol_progress, SyncState};
use crate::services::sync_types::GapProgress;

use chrono::{Duration, Local, Months};
use std::collections::HashSet;
use std::{sync::Arc, time};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

const KEY_ENABLED: &str = "daily_sync_enabled";
const KEY_TIME: &str = "daily_sync_time";
const KEY_LAST_RUN_DATE: &str = "daily_sync_last_run_date";

pub async fn run_daily_scheduler(state: Arc<AppState>, cancel: CancellationToken) {
    loop {
        let now = Local::now();
        let today = now.format(FINMIND_DATE_FORMAT).to_string();
        let current_hm = now.format("%H:%M").to_string();

        let mut redis = state.redis_client.clone();
        let enabled: Option<String> = redis::cmd("GET")
            .arg(KEY_ENABLED)
            .query_async(&mut redis)
            .await
            .ok()
            .flatten();
        let time: Option<String> = redis::cmd("GET")
            .arg(KEY_TIME)
            .query_async(&mut redis)
            .await
            .ok()
            .flatten();
        let last_run: Option<String> = redis::cmd("GET")
            .arg(KEY_LAST_RUN_DATE)
            .query_async(&mut redis)
            .await
            .ok()
            .flatten();

        let enabled = enabled.as_deref() == Some("true");
        let schedule_time = time.unwrap_or_else(|| "02:00".to_string());

        if enabled && current_hm == schedule_time && last_run.as_deref() != Some(&today) {
            run_scheduled_pipeline(&state, &now).await;

            let _: redis::RedisResult<()> = redis::cmd("SET")
                .arg(KEY_LAST_RUN_DATE)
                .arg(&today)
                .query_async(&mut redis)
                .await;
        }

        tokio::select! {
            _ = tokio::time::sleep(time::Duration::from_secs(30)) => {}
            _ = cancel.cancelled() => {
                tracing::info!("Daily scheduler stopped");
                return;
            }
        }
    }
}

/// 排程器的有序同步 pipeline。
///
/// 執行順序固定：
///   1. StockInfo              — 更新股票基本資料（名稱、交易所）
///   2. TradingDate            — 更新交易日曆快取
///   3. TaiwanStockPrice       — 補齊日K缺口
///   4. InstitutionalInvestors — 補齊三大法人缺口
async fn run_scheduled_pipeline(state: &Arc<AppState>, now: &chrono::DateTime<Local>) {
    let sync_id = format!("sync-{}-scheduled", now.format("%Y%m%d%H%M"));
    let started_at_ms = current_timestamp_ms();

    // ── Step 1: StockInfo（全市場，不需要 symbol 迴圈）────────────────────────
    {
        let mut redis = state.redis_client.clone();
        let mut buffer = state.bulk_insert_buffer.lock().await;
        let dummy_gap = DateRange {
            from_date: chrono::Utc::now().date_naive() - Duration::days(1),
            to_date: chrono::Utc::now().date_naive(),
        };
        let mut ctx = SyncContext {
            db_pool: &state.db_pool,
            http_client: &state.http_client,
            rate_limiter: &state.rate_limiter,
            redis: &mut redis,
            buffer: &mut buffer,
            sync_id: &sync_id,
            trading_dates: &HashSet::new(),
        };
        if let Err(e) = StockInfoDataset
            .fetch_and_insert("", &dummy_gap, &mut ctx)
            .await
        {
            warn!(error = %e, sync_id = %sync_id, "StockInfo sync failed, continuing pipeline");
        } else {
            info!(sync_id = %sync_id, "StockInfo sync complete");
        }
    }

    // ── Step 2: TradingDate（全範圍刷新快取）─────────────────────────────────
    {
        let today_date = Local::now().date_naive();
        let from_date = today_date
            .checked_sub_months(Months::new(5 * 12))
            .expect("日期計算超出範圍");
        let gap = DateRange {
            from_date,
            to_date: today_date,
        };
        let mut redis = state.redis_client.clone();
        let mut buffer = state.bulk_insert_buffer.lock().await;
        let mut ctx = SyncContext {
            db_pool: &state.db_pool,
            http_client: &state.http_client,
            rate_limiter: &state.rate_limiter,
            redis: &mut redis,
            buffer: &mut buffer,
            sync_id: &sync_id,
            trading_dates: &HashSet::new(),
        };
        if let Err(e) = TradingDateDataset
            .fetch_and_insert("", &gap, &mut ctx)
            .await
        {
            warn!(error = %e, sync_id = %sync_id, "TradingDate sync failed, continuing pipeline");
        } else {
            info!(sync_id = %sync_id, "TradingDate sync complete");
        }
    }

    // ── 取得 active symbols，供後續 dataset 使用 ─────────────────────────────
    let active_symbols = match fetch_active_symbols(&state.db_pool, None).await {
        Ok(s) if !s.is_empty() => s,
        Ok(_) => {
            warn!(
                sync_id = %sync_id,
                "No active symbols found, skipping candles and institutional sync"
            );
            return;
        }
        Err(e) => {
            error!(error = %e, sync_id = %sync_id, "Failed to fetch active symbols");
            return;
        }
    };

    let scope = SyncScope {
        full_sync: false,
        from_date: Some(chrono::Utc::now().date_naive() - Duration::days(7)),
        to_date: Some(chrono::Utc::now().date_naive()),
    };

    // ── 載入交易日曆（Step 2 已更新快取，這裡直接讀）────────────────────────
    let trading_dates = {
        let mut redis = state.redis_client.clone();
        match load_trading_dates_5y(&state.http_client, &state.rate_limiter, &mut redis).await {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    error = %e,
                    sync_id = %sync_id,
                    "Failed to load trading dates, using empty set"
                );
                HashSet::new()
            }
        }
    };

    // ── 建立 sync_log 紀錄 ────────────────────────────────────────────────────
    if let Err(e) = sync_log_create(
        &state.db_pool,
        &SyncLogEntry {
            sync_id: sync_id.clone(),
            sync_type: "scheduled".to_string(),
            triggered_by: "system".to_string(),
            symbols: active_symbols.clone(),
        },
        started_at_ms,
    )
    .await
    {
        warn!(error = %e, sync_id = %sync_id, "Failed to create sync_log entry");
    }

    // ── 建立 Redis 初始狀態 ───────────────────────────────────────────────────
    let initial_state = SyncState::new(sync_id.clone(), active_symbols.clone(), started_at_ms);
    {
        let mut redis = state.redis_client.clone();
        if let Err(e) = save_sync_state(&mut redis, &initial_state).await {
            warn!(error = %e, sync_id = %sync_id, "Failed to save initial sync state");
        }
    }

    // ── 有序 pipeline：Step 3 和 Step 4 共用相同的 symbol 迴圈結構 ───────────
    let dataset_pipeline: Vec<Box<dyn DatasetSync>> = vec![
        Box::new(CandlesDataset::default()),     // Step 3
        Box::new(InstitutionalInvestorsDataset), // Step 4
    ];

    let mut has_error = false;

    for symbol in &active_symbols {
        // ── 標記 symbol 為執行中 ──────────────────────────────────────────────
        {
            let mut redis = state.redis_client.clone();
            let _ = update_symbol_progress(
                &mut redis,
                &sync_id,
                symbol,
                SymbolSyncStatus::Running,
                None,
                None,
            )
            .await;
        }

        let mut symbol_failed = false;
        let mut result_gap_a: Option<GapProgress> = None;
        let mut result_gap_b: Option<GapProgress> = None;

        for syncer in &dataset_pipeline {
            // ── 缺口偵測（block 結束後釋放借用）─────────────────────────────
            let detect_result = {
                let mut redis = state.redis_client.clone();
                let mut buffer = state.bulk_insert_buffer.lock().await;
                let ctx = SyncContext {
                    db_pool: &state.db_pool,
                    http_client: &state.http_client,
                    rate_limiter: &state.rate_limiter,
                    redis: &mut redis,
                    buffer: &mut buffer,
                    sync_id: &sync_id,
                    trading_dates: &trading_dates,
                };
                syncer.detect_gaps(&scope, symbol, &ctx).await
            };

            let (gap_a, gap_b) = match detect_result {
                Ok(g) => g,
                Err(e) => {
                    error!(
                        error = %e,
                        symbol = %symbol,
                        dataset = %syncer.name(),
                        "detect_gaps failed"
                    );
                    symbol_failed = true;
                    has_error = true;
                    continue;
                }
            };

            for (gap_opt, is_gap_a) in [(&gap_a, true), (&gap_b, false)] {
                if let Some(gap) = gap_opt {
                    let mut redis = state.redis_client.clone();
                    let mut buffer = state.bulk_insert_buffer.lock().await;
                    let mut ctx = SyncContext {
                        db_pool: &state.db_pool,
                        http_client: &state.http_client,
                        rate_limiter: &state.rate_limiter,
                        redis: &mut redis,
                        buffer: &mut buffer,
                        sync_id: &sync_id,
                        trading_dates: &trading_dates,
                    };
                    match syncer.fetch_and_insert(symbol, gap, &mut ctx).await {
                        Ok(r) => {
                            if r.inserted > 0 || r.skipped > 0 {
                                sync_log_update_counts(
                                    &state.db_pool,
                                    &sync_id,
                                    r.inserted,
                                    r.skipped,
                                    0,
                                )
                                .await
                                .unwrap_or_else(|e| warn!(error = %e, "sync_log update failed"));
                            }
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
                            if is_gap_a {
                                result_gap_a = Some(progress);
                            } else {
                                result_gap_b = Some(progress);
                            }
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                symbol = %symbol,
                                dataset = %syncer.name(),
                                "fetch_and_insert failed"
                            );
                            symbol_failed = true;
                            has_error = true;
                        }
                    }
                }
            }
        }

        // ── 所有 dataset 跑完才標最終狀態 ────────────────────────────────────
        let symbol_status = if symbol_failed {
            SymbolSyncStatus::Failed
        } else {
            SymbolSyncStatus::Completed
        };
        let mut redis = state.redis_client.clone();
        let _ = update_symbol_progress(
            &mut redis,
            &sync_id,
            symbol,
            symbol_status,
            result_gap_a,
            result_gap_b,
        )
        .await;
    }

    // ── 最終狀態寫回 DB ───────────────────────────────────────────────────────
    let final_status = if has_error {
        SyncStatus::Failed.as_str()
    } else {
        SyncStatus::Completed.as_str()
    };

    if let Err(e) = sync_log_update_status(
        &state.db_pool,
        &sync_id,
        final_status,
        Some(current_timestamp_ms()),
    )
    .await
    {
        warn!(error = %e, sync_id = %sync_id, "Failed to update sync_log final status");
    }

    info!(sync_id = %sync_id, status = %final_status, "Scheduled pipeline complete");
}
