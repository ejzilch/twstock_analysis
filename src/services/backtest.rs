/// BacktestService — 回測業務流程協調者（Service layer）
///
/// 職責：
///   1. 從 DB 取得 K 線資料
///   2. 驗證請求參數
///   3. 呼叫純計算 BacktestEngine
///   4. 組裝 BacktestResponse
///
/// handler 只需呼叫 `BacktestService::run()`，不再包含任何業務邏輯。
use crate::api::backtest::dto::{request::BacktestRequest, response::BacktestResponse};
use crate::api::middleware::ApiError;
use crate::app_state::AppState;
use crate::domain::backtest::engine::{run as engine_run, BacktestInput};
use crate::models::candle::CandleRow;

pub struct BacktestService;

impl BacktestService {
    /// 執行完整回測流程。
    pub async fn run(
        state: &AppState,
        request: &BacktestRequest,
    ) -> Result<BacktestResponse, ApiError> {
        // ── Step 1: 參數驗證 ──────────────────────────────────────────────────
        Self::validate_request(request)?;

        // ── Step 2: 取得 K 線資料 ─────────────────────────────────────────────
        let candles = Self::fetch_candles(state, request).await?;

        if candles.len() < 3 {
            return Err(ApiError::InvalidIndicatorConfig {
                detail: "not enough candle data for backtest (need at least 3 daily candles)"
                    .to_string(),
            });
        }

        // ── Step 3: 執行引擎（純計算） ────────────────────────────────────────
        let input = BacktestInput {
            candles: &candles,
            strategy_name: request.strategy_name.clone(),
            initial_capital: request.initial_capital,
            position_size_percent: request.position_size_percent,
            exit_filter_pct: request.exit_filter_pct,
            min_holding_days: request.min_holding_days,
        };

        let output = engine_run(&input).map_err(|e| ApiError::InvalidIndicatorConfig {
            detail: e.to_string(),
        })?;

        // ── Step 4: 組裝 Response ─────────────────────────────────────────────
        Ok(BacktestResponse {
            backtest_id: format!("bt-{}", uuid::Uuid::new_v4()),
            symbol: request.symbol.clone(),
            strategy_name: request.strategy_name.clone(),
            from_ms: request.from_ms,
            to_ms: request.to_ms,
            initial_capital: request.initial_capital,
            final_capital: output.final_capital,
            exit_filter_pct: output.exit_filter_pct,
            trades: output.trades,
            metrics: output.metrics,
            created_at_ms: output.created_at_ms,
        })
    }

    // ── 私有方法 ──────────────────────────────────────────────────────────────

    fn validate_request(request: &BacktestRequest) -> Result<(), ApiError> {
        if request.initial_capital <= 0.0 {
            return Err(ApiError::InvalidIndicatorConfig {
                detail: "initial_capital must be greater than 0".to_string(),
            });
        }
        if !(1.0..=100.0).contains(&request.position_size_percent) {
            return Err(ApiError::InvalidIndicatorConfig {
                detail: "position_size_percent must be between 1 and 100".to_string(),
            });
        }
        if request.from_ms >= request.to_ms {
            return Err(ApiError::InvalidIndicatorConfig {
                detail: "from_ms must be earlier than to_ms".to_string(),
            });
        }
        Ok(())
    }

    async fn fetch_candles(
        state: &AppState,
        request: &BacktestRequest,
    ) -> Result<Vec<CandleRow>, ApiError> {
        let candles = sqlx::query_as::<_, CandleRow>(
            r#"
            SELECT timestamp_ms, close
            FROM candles
            WHERE symbol = $1
              AND interval = '1d'
              AND timestamp_ms >= $2
              AND timestamp_ms <= $3
            ORDER BY timestamp_ms ASC
            "#,
        )
        .bind(&request.symbol)
        .bind(request.from_ms)
        .bind(request.to_ms)
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Backtest candle query failed: {e}")))?;

        Ok(candles)
    }
}
