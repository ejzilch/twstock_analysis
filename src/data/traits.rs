use crate::data::models::RawCandle;
use crate::domain::BridgeError;
use async_trait::async_trait;

#[async_trait]
pub trait DbWriter: Send + Sync {
    async fn write_batch(&self, batch: &[RawCandle]) -> Result<usize, BridgeError>;
}

// 純同步，不用 async_trait
pub trait CacheInvalidator: Send {
    fn invalidate(&mut self, symbols: &[String]);
}
