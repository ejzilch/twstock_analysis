use crate::app_state::AppState;
use crate::constants::FINMIND_DATE_FORMAT;
use crate::data::symbol_sync::{fetch_active_symbols, refresh_symbols_from_finmind};
use crate::models::enums::{SymbolFetchScope, SyncMode};
use crate::services::admin_sync::{StartSyncRequest, SyncService};

use chrono::{Local, Months};
use std::{sync::Arc, time};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

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
            // 全市場同步：only_missing = false，確保每日排程都拿到最新 metadata
            let symbols = match refresh_symbols_from_finmind(
                &state.db_pool,
                &state.http_client,
                &state.rate_limiter,
                SymbolFetchScope::AllMarkets,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "Daily scheduler failed to refresh symbols; skipping this run");
                    let should_stop = tokio::select! {
                        _ = tokio::time::sleep(time::Duration::from_secs(30)) => false,
                        _ = cancel.cancelled() => true,
                    };
                    if should_stop {
                        tracing::info!("Daily scheduler stopped (error recovery)");
                        return;
                    }
                    continue;
                }
            };

            let active_symbols = match fetch_active_symbols(&state.db_pool, None).await {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "Failed to fetch active symbols; skipping this run");
                    let should_stop = tokio::select! {
                        _ = tokio::time::sleep(time::Duration::from_secs(30)) => false,
                        _ = cancel.cancelled() => true,
                    };
                    if should_stop {
                        tracing::info!("Daily scheduler stopped (error recovery)");
                        return;
                    }
                    continue;
                }
            };

            if active_symbols.is_empty() {
                warn!("Daily scheduler skipped because no symbols were resolved from FinMind");
            } else {
                info!(
                    total = symbols.len(),
                    "Daily scheduler refreshed FinMind symbols"
                );

                let from_date = now
                    .date_naive()
                    .checked_sub_months(Months::new(60))
                    .map(|d| d.format(FINMIND_DATE_FORMAT).to_string());
                let to_date = Some(now.date_naive().format(FINMIND_DATE_FORMAT).to_string());

                let _ = SyncService::start(
                    &state,
                    StartSyncRequest {
                        request_id: format!("scheduled-{}", now.timestamp_millis()),
                        mode: SyncMode::Partial,
                        symbols: Some(symbols),
                        full_sync: false,
                        from_date,
                        to_date,
                        intervals: vec![],
                    },
                )
                .await;
            }

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
