/// src/data/mocks.rs
///
/// 測試專用 mock 實作。
/// 只在 #[cfg(test)] 或 test feature 下編譯。
///
/// InMemoryDbWriter   — 記錄寫入的資料，供驗收斷言
/// SpyCacheInvalidator — 記錄 invalidate 呼叫，供驗收斷言
#[cfg(test)]
pub mod test_mocks {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use crate::core::BridgeError;
    use crate::data::models::RawCandle;
    use crate::data::traits::{CacheInvalidator, DbWriter};

    // ── InMemoryDbWriter ──────────────────────────────────────────────────────

    /// 將 write_batch 的輸入記錄到 Vec，供測試驗收。
    /// 可選擇性設定 should_fail，模擬 DB 錯誤。
    #[derive(Clone, Default)]
    pub struct InMemoryDbWriter {
        pub written:     Arc<Mutex<Vec<RawCandle>>>,
        pub should_fail: bool,
        /// 模擬 ON CONFLICT DO NOTHING：跳過 symbol 在此 set 中的資料
        pub conflict_symbols: Arc<Mutex<Vec<String>>>,
    }

    impl InMemoryDbWriter {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_failure() -> Self {
            Self { should_fail: true, ..Default::default() }
        }

        /// 取出已寫入的資料快照。
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
                    source:  None,
                });
            }

            let conflicts = self.conflict_symbols.lock().unwrap();
            let mut written = self.written.lock().unwrap();
            let mut count = 0usize;

            for candle in batch {
                // 模擬 ON CONFLICT DO NOTHING
                if conflicts.contains(&candle.symbol) {
                    continue;
                }
                written.push(candle.clone());
                count += 1;
            }
            Ok(count)
        }
    }

    // ── SpyCacheInvalidator ───────────────────────────────────────────────────

    /// 記錄 invalidate() 的呼叫次數與傳入的 symbols，供測試驗收。
    #[derive(Default)]
    pub struct SpyCacheInvalidator {
        /// 所有呼叫中傳入的 symbols，展平後的清單
        pub invalidated_symbols: Vec<String>,
        /// invalidate() 被呼叫的次數
        pub call_count: usize,
    }

    impl CacheInvalidator for SpyCacheInvalidator {
        fn invalidate(&mut self, symbols: &[String]) {
            self.call_count += 1;
            self.invalidated_symbols.extend_from_slice(symbols);
        }
    }
}
