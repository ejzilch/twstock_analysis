use crate::data::db::BulkInsertBuffer;
use crate::data::implementations::{PostgresDbWriter, RedisInvalidator};
use crate::AiServiceClient;
use crate::FinMindRateLimiter;
use redis::aio::MultiplexedConnection;
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct AppState {
    pub db_pool: PgPool,
    pub redis_client: MultiplexedConnection,
    pub ai_client: AiServiceClient,
    pub rate_limiter: Arc<FinMindRateLimiter>,
    pub http_client: Client,
    pub bulk_insert_buffer: Arc<Mutex<BulkInsertBuffer>>,
}

impl AppState {
    pub fn db_writer(&self) -> PostgresDbWriter {
        PostgresDbWriter::new(self.db_pool.clone())
    }

    pub fn cache_invalidator(&self) -> Result<RedisInvalidator, redis::RedisError> {
        let conn = self.redis_client.clone();
        Ok(RedisInvalidator::new(conn))
    }
}
