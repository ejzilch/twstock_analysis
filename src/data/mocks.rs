/// src/data/mocks.rs
///
/// 測試專用 mock 實作。
/// 只在 #[cfg(test)] 或 test feature 下編譯。
///
/// InMemoryDbWriter   — 記錄寫入的資料，供驗收斷言
/// SpyCacheInvalidator — 記錄 invalidate 呼叫，供驗收斷言
#[cfg(test)]
pub mod test_mocks {
    use crate::data::models::RawCandle;
    use crate::data::traits::{CacheInvalidator, DbWriter};
    use crate::domain::BridgeError;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    pub struct InMemoryDbWriter {
        pub written: Arc<Mutex<Vec<RawCandle>>>,
        pub should_fail: bool,
        pub conflict_symbols: Arc<Mutex<Vec<String>>>,
    }

    impl InMemoryDbWriter {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn with_failure() -> Self {
            Self {
                should_fail: true,
                ..Default::default()
            }
        }
        pub fn written_snapshot(&self) -> Vec<RawCandle> {
            self.written.lock().unwrap().clone()
        }
        pub fn written_count(&self) -> usize {
            self.written.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl DbWriter for InMemoryDbWriter {
        async fn write_batch(&self, batch: &[RawCandle]) -> Result<usize, BridgeError> {
            if self.should_fail {
                return Err(BridgeError::FinMindDataSourceError {
                    context: "Simulated DB failure".into(),
                    source: None,
                });
            }
            let conflicts = self.conflict_symbols.lock().unwrap();
            let mut written = self.written.lock().unwrap();
            let mut count = 0usize;
            for candle in batch {
                if conflicts.contains(&candle.symbol) {
                    continue;
                }
                written.push(candle.clone());
                count += 1;
            }
            Ok(count)
        }
    }

    #[derive(Default)]
    pub struct SpyCacheInvalidator {
        pub invalidated_symbols: Vec<String>,
        pub call_count: usize,
    }

    // 純同步，對應 trait
    impl CacheInvalidator for SpyCacheInvalidator {
        fn invalidate(&mut self, symbols: &[String]) {
            self.call_count += 1;
            self.invalidated_symbols.extend_from_slice(symbols);
        }
    }
}
