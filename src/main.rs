mod ai_client;
mod api;
mod constants;
mod core;
mod data;
mod models;

use crate::ai_client::AiServiceClient;
use crate::api::middleware::rate_limit::new_rate_limiter_state;
use crate::api::{build_router, handlers::AppState};
use crate::data::{
    db::{BulkInsertBuffer, DbClient},
    fetch_rate_limiter::{ApiTier, FinMindRateLimiter, RateLimitConfig},
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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

    tracing::info!("Redis client initialized");

    // 初始化 FinMind Rate Limiter
    let finmind_tier = match std::env::var("FINMIND_API_TIER")
        .unwrap_or_else(|_| "free".to_string())
        .as_str()
    {
        "paid" => ApiTier::Paid,
        _ => ApiTier::Free,
    };

    let rate_limit_config = match finmind_tier {
        ApiTier::Paid => RateLimitConfig::paid(),
        ApiTier::Free => RateLimitConfig::free(),
    };

    let finmind_rate_limiter = Arc::new(FinMindRateLimiter::new(rate_limit_config));
    tracing::info!(tier = ?finmind_tier, "FinMind rate limiter initialized");

    // 初始化 DbClient
    let db_client = Arc::new(DbClient {
        pool: pg_pool.clone(),
        redis_client: redis_client.clone(),
    });

    // 初始化 BulkInsertBuffer
    let bulk_insert_buffer = Arc::new(tokio::sync::Mutex::new(BulkInsertBuffer::new()));

    // 初始化 Python AI Service 客戶端
    let ai_service_url = std::env::var("PYTHON_AI_SERVICE_URL")
        .expect("PYTHON_AI_SERVICE_URL must be set in environment");

    let ai_client =
        Arc::new(AiServiceClient::new(ai_service_url).expect("Failed to create AI service client"));

    tracing::info!("AI service client initialized");

    // 組裝應用程式狀態
    let app_state = Arc::new(AppState {
        db_client: db_client.clone(),
        ai_client: ai_client.clone(),
        rate_limiter: finmind_rate_limiter,
    });

    // 組裝 Router
    let rate_limiter_state = new_rate_limiter_state();
    let app = build_router(app_state, rate_limiter_state);

    // 啟動 TCP 監聽
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8089".to_string());
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
        let mut buffer = bulk_insert_buffer.lock().await;
        if let Err(e) = buffer.flush_and_close(&db_client).await {
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

    if app_env == "production" {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().pretty())
            .init();
    }
}
