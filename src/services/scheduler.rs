use crate::app_state::AppState;
use crate::constants::FINMIND_API_TOKEN_ENV;
use crate::data::fetch::fetch_stock_info_map;
use crate::data::models::current_timestamp_ms;
use crate::data::symbol_sync::{upsert_symbols, SymbolSyncData};
use crate::models::enums::{DataSource, SyncMode};
use crate::services::admin_sync::{StartSyncRequest, SyncService};
use chrono::{Local, Months};
use std::sync::Arc;
use tracing::{error, info, warn};

const KEY_ENABLED: &str = "daily_sync_enabled";
const KEY_TIME: &str = "daily_sync_time";
const KEY_LAST_RUN_DATE: &str = "daily_sync_last_run_date";

pub async fn run_daily_scheduler(state: Arc<AppState>) {
    loop {
        let now = Local::now();
        let today = now.format("%Y-%m-%d").to_string();
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
            let all_symbols = sync_all_market_symbols(&state).await;

            if all_symbols.is_empty() {
                warn!("Daily scheduler skipped because no symbols were resolved from FinMind");
            } else {
                let from_date = now
                    .date_naive()
                    .checked_sub_months(Months::new(60))
                    .map(|d| d.format("%Y-%m-%d").to_string());
                let to_date = Some(now.date_naive().format("%Y-%m-%d").to_string());

                let _ = SyncService::start(
                    &state,
                    StartSyncRequest {
                        request_id: format!("scheduled-{}", now.timestamp_millis()),
                        mode: SyncMode::Partial,
                        symbols: Some(all_symbols),
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

        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

async fn sync_all_market_symbols(state: &AppState) -> Vec<String> {
    let api_token = std::env::var(FINMIND_API_TOKEN_ENV).unwrap_or_default();
    let now_ms = current_timestamp_ms();

    let stock_info_map = match fetch_stock_info_map(&state.http_client, &api_token).await {
        Ok(map) => map,
        Err(err) => {
            error!(error = %err, "Failed to fetch TaiwanStockInfo for daily scheduler");
            return vec![];
        }
    };

    let mut symbols = stock_info_map.keys().cloned().collect::<Vec<_>>();
    symbols.sort();

    let upsert_payload = symbols
        .iter()
        .filter_map(|symbol| {
            stock_info_map.get(symbol).map(|info| SymbolSyncData {
                symbol: symbol.clone(),
                name: info.name.clone(),
                exchange: info.exchange,
                data_source: DataSource::FinMind,
                finmind_earliest_ms: None,
                latest_ms: now_ms,
                is_active: true,
            })
        })
        .collect::<Vec<_>>();

    match upsert_symbols(&state.db_pool, &upsert_payload, now_ms).await {
        Ok(summary) => {
            info!(
                inserted = summary.inserted,
                updated = summary.updated,
                failed = summary.failed,
                total = symbols.len(),
                "Daily scheduler refreshed FinMind symbols"
            );
            symbols
        }
        Err(err) => {
            error!(error = %err, "Failed to upsert symbols before daily sync");
            vec![]
        }
    }
}
