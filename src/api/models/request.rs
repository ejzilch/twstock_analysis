use crate::models::Exchange;
use serde::Deserialize;

/// GET /api/v1/symbols 的查詢參數
#[derive(Debug, Clone, Deserialize)]
pub struct SymbolsQueryParams {
    /// 交易所篩選：TWSE / TPEX
    pub exchange: Option<Exchange>,
    /// 是否只回傳上市中的標的，預設 true
    pub is_active: Option<bool>,
}

impl SymbolsQueryParams {
    /// is_active 缺省時預設為 true
    pub fn is_active(&self) -> bool {
        self.is_active.unwrap_or(true)
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbols_query_is_active_default() {
        let params = SymbolsQueryParams {
            exchange: None,
            is_active: None,
        };
        assert!(params.is_active());
    }
}
