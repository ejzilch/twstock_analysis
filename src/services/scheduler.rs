use crate::app_state::AppState;
use crate::models::enums::SyncMode;
use crate::services::admin_sync::{StartSyncRequest, SyncService};
use chrono::Local;
use std::sync::Arc;

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
            let _ = SyncService::start(
                &state,
                StartSyncRequest {
                    request_id: format!("scheduled-{}", now.timestamp_millis()),
                    mode: SyncMode::All,
                    symbols: None,
                    full_sync: true,
                    from_date: None,
                    to_date: None,
                    intervals: vec![],
                },
            )
            .await;
            let _: redis::RedisResult<()> = redis::cmd("SET")
                .arg(KEY_LAST_RUN_DATE)
                .arg(&today)
                .query_async(&mut redis)
                .await;
        }

        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}
