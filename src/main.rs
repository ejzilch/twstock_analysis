mod ai_client;
mod api;
mod app_state;
mod constants;
mod data;
mod domain;
mod models;
mod services;
use crate::ai_client::AiServiceClient;
use crate::api::build_router;
use crate::api::middleware::rate_limit::new_rate_limiter_state;
use crate::app_state::AppState;
use crate::data::{
    db::BulkInsertBuffer,
    fetch_rate_limiter::{ApiTier, FinMindRateLimiter},
};
use std::sync::Arc;
use tokio::net::TcpListener;

use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 載入 .env 檔案
    dotenvy::dotenv().ok();

    // 初始化 tracing
    init_tracing();

    tracing::info!("Starting AI Bridge API server");

    // 初始化 PostgreSQL 連線池
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment");

    let pg_pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");

    tracing::info!("PostgreSQL connection pool initialized");

    // 初始化 Redis 客戶端
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set in environment");

    let redis_client = redis::Client::open(redis_url).expect("Failed to create Redis client");

    let multiplexed_conn = redis_client.get_multiplexed_async_connection().await?;

    tracing::info!("Redis client initialized");

    // 初始化 FinMind Rate Limiter
    let finmind_tier = match std::env::var("FINMIND_API_TIER")
        .unwrap_or_else(|_| "free".to_string())
        .as_str()
    {
        "paid" => ApiTier::Paid,
        _ => ApiTier::Free,
    };

    let finmind_rate_limiter = FinMindRateLimiter::new(finmind_tier);
    tracing::info!(tier = ?finmind_tier, "FinMind rate limiter initialized");

    // 初始化 BulkInsertBuffer
    let bulk_insert_buffer = Arc::new(tokio::sync::Mutex::new(BulkInsertBuffer::new()));

    // 初始化 Python AI Service 客戶端
    let ai_service_url = std::env::var("PYTHON_AI_SERVICE_URL")
        .expect("PYTHON_AI_SERVICE_URL must be set in environment");

    let ai_client =
        AiServiceClient::new(ai_service_url).expect("Failed to create AI service client");

    tracing::info!("AI service client initialized");

    // 初始化 HTTP Client (AppState 需要這個)
    let http_client = reqwest::Client::new();

    // 直接組裝 AppState
    let app_state = Arc::new(AppState {
        db_pool: pg_pool.clone(), // 直接傳入 pg_pool
        redis_client: multiplexed_conn.clone(),
        ai_client: ai_client.clone(),
        rate_limiter: finmind_rate_limiter,
        http_client, // 傳入剛剛建立的 http_client
        bulk_insert_buffer: bulk_insert_buffer.clone(),
    });

    // 組裝 Router
    let rate_limiter_state = new_rate_limiter_state();
    let app = build_router(app_state.clone(), rate_limiter_state);

    // 啟動 TCP 監聽
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8089".to_string());
    let listener = TcpListener::bind(&bind_addr).await?;

    tracing::info!(addr = %bind_addr, "Server listening");

    // Graceful Shutdown 信號
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for shutdown signal");
        tracing::info!("Shutdown signal received, starting graceful shutdown");
    };

    // 啟動 Axum server，掛載 graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    // Axum 完成所有進行中請求後，依序清理資源
    tracing::info!("All in-flight requests completed");

    // 步驟 3: Flush BulkInsertBuffer 剩餘資料
    {
        let mut buffer = app_state.bulk_insert_buffer.lock().await;
        let mut redis_conn = app_state.cache_invalidator()?;
        if let Err(e) = buffer
            .flush_and_close(&app_state.db_writer(), &mut redis_conn)
            .await
        {
            tracing::error!(error = %e, "Failed to flush BulkInsertBuffer during shutdown");
        }
    }

    // 步驟 4: 關閉 PostgreSQL 連線池
    pg_pool.close().await;
    tracing::info!("PostgreSQL connection pool closed");

    tracing::info!("Graceful shutdown complete");
    Ok(())
}

/// 初始化 tracing subscriber
///
/// APP_ENV=production: JSON 格式結構化 log，適合 ELK / Grafana Loki
/// APP_ENV=development（預設）: 人類可讀格式，適合本機開發
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

    // 設定時間格式為本地時間 (例如：2026-04-22 17:37:42)
    // 格式定義字串：[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]
    let timer = fmt::time::LocalTime::new(time::macros::format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
    ));

    if app_env == "production" {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().json().with_timer(timer))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().pretty().with_timer(timer))
            .init();
    }
}
