use anyhow::Ok;

/// BacktestService — 回測業務流程協調者（Service layer）
///
/// 職責：
///   1. 從 DB 取得 K 線資料
///   2. 驗證請求參數
///   3. 呼叫純計算 BacktestEngine
///   4. 組裝 BacktestResponse
///
/// handler 只需呼叫 `BacktestService::run()`，不再包含任何業務邏輯。
use crate::app_state::AppState;
use crate::domain::backtest::engine::{run as engine_run, BacktestInput, BacktestOutput};
use crate::models::candle::CandleRow;

// service 層 model
pub struct BacktestParams {
    pub symbol: String,
    pub strategy_name: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub initial_capital: f64,
    pub position_size_percent: f64,
    pub exit_filter_pct: Option<f64>,
    pub min_holding_days: Option<u32>,
}

pub struct BacktestService;

impl BacktestService {
    /// 執行完整回測流程。
    pub async fn run(
        state: &AppState,
        params: &BacktestParams,
    ) -> Result<BacktestOutput, anyhow::Error> {
        // ── Step 1: 參數驗證 ──────────────────────────────────────────────────
        Self::validate_request(params)?;

        // ── Step 2: 取得 K 線資料 ─────────────────────────────────────────────
        let candles = Self::fetch_candles(state, params).await?;

        if candles.len() < 3 {
            return Err(anyhow::anyhow!(
                "not enough candle data for backtest (need at least 3 daily candles)"
            ));
        }

        // ── Step 3: 執行引擎（純計算） ────────────────────────────────────────
        let input = BacktestInput {
            candles: &candles,
            strategy_name: params.strategy_name.clone(),
            initial_capital: params.initial_capital,
            position_size_percent: params.position_size_percent,
            exit_filter_pct: params.exit_filter_pct,
            min_holding_days: params.min_holding_days,
        };

        let output = engine_run(&input).map_err(anyhow::Error::msg)?;

        // ── Step 4: 組裝 Response ─────────────────────────────────────────────
        // Ok(BacktestResponse {
        //     backtest_id: format!("bt-{}", uuid::Uuid::new_v4()),
        //     symbol: params.symbol.clone(),
        //     strategy_name: params.strategy_name.clone(),
        //     from_ms: params.from_ms,
        //     to_ms: params.to_ms,
        //     initial_capital: params.initial_capital,
        //     final_capital: output.final_capital,
        //     exit_filter_pct: output.exit_filter_pct,
        //     trades: output.trades,
        //     metrics: output.metrics,
        //     created_at_ms: output.created_at_ms,
        // })
        Ok(output)
    }

    // ── 私有方法 ──────────────────────────────────────────────────────────────

    fn validate_request(params: &BacktestParams) -> Result<(), anyhow::Error> {
        if params.initial_capital <= 0.0 {
            return Err(anyhow::anyhow!("initial_capital must be greater than 0"));
        }
        if !(1.0..=100.0).contains(&params.position_size_percent) {
            return Err(anyhow::anyhow!(
                "position_size_percent must be between 1 and 100".to_string()
            ));
        }
        if params.from_ms >= params.to_ms {
            return Err(anyhow::anyhow!(
                "from_ms must be earlier than to_ms".to_string()
            ));
        }
        Ok(())
    }

    async fn fetch_candles(
        state: &AppState,
        params: &BacktestParams,
    ) -> Result<Vec<CandleRow>, anyhow::Error> {
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
        .bind(&params.symbol)
        .bind(params.from_ms)
        .bind(params.to_ms)
        .fetch_all(&state.db_pool)
        .await
        .map_err(|e| anyhow::anyhow!("Backtest candle query failed: {e}"))?;

        Ok(candles)
    }
}
