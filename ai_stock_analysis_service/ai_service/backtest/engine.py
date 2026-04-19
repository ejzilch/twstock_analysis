"""
Backtest engine.
CRITICAL: All technical indicators fetched from Rust via POST /api/v1/indicators/compute.
          Never self-compute any indicator — this ensures parity with live signals.
"""
import time
import logging
import math
from typing import Any

import httpx

from ai_service.constants import (
    RUST_API_BASE_URL,
    RUST_API_TIMEOUT_SECS,
    INDICATORS_ENDPOINT,
)
from ai_service.predict.validator import validate_backtest_metrics

logger = logging.getLogger(__name__)


async def run_backtest(
    symbol:                str,
    strategy_name:         str,
    from_ms:               int,
    to_ms:                 int,
    initial_capital:       float,
    position_size_percent: int,
    request_id:            str,
    api_key:               str,
) -> dict[str, Any]:
    """
    Execute backtest for a given strategy and time range.
    Fetches indicators from Rust, then simulates trade execution.
    """
    # Step 1: Fetch indicators from Rust (enforced — never self-compute)
    indicators_payload = await _fetch_indicators(symbol, from_ms, to_ms, request_id, api_key)

    candle_count = len(indicators_payload.get("indicators", {}).get("ma20", []))
    if candle_count == 0:
        raise ValueError(f"No indicator data returned for {symbol} in given range")

    # Step 2: Run strategy simulation
    trades = _simulate_strategy(
        strategy_name=strategy_name,
        indicators=indicators_payload["indicators"],
        initial_capital=initial_capital,
        position_size_percent=position_size_percent,
    )

    # Step 3: Compute performance metrics
    metrics = _compute_metrics(trades, initial_capital)
    validate_backtest_metrics(metrics)

    final_capital = initial_capital + sum(t["pnl"] for t in trades)

    backtest_id = f"bt-{request_id}"
    return {
        "backtest_id":      backtest_id,
        "symbol":           symbol,
        "strategy_name":    strategy_name,
        "from_ms":          from_ms,
        "to_ms":            to_ms,
        "initial_capital":  initial_capital,
        "final_capital":    final_capital,
        "metrics":          metrics,
        "created_at_ms":    int(time.time() * 1000),
    }


async def _fetch_indicators(
    symbol: str,
    from_ms: int,
    to_ms: int,
    request_id: str,
    api_key: str,
) -> dict[str, Any]:
    """
    Call Rust POST /api/v1/indicators/compute to get all required indicators.
    Raises httpx.HTTPError on failure — never falls back to self-computation.
    """
    payload = {
        "request_id": request_id,
        "symbol":     symbol,
        "from_ms":    from_ms,
        "to_ms":      to_ms,
        "interval":   "15m",
        "indicators": {
            "ma":   [5, 20, 50],
            "rsi":  [14],
            "macd": [12, 26, 9],
            "bollinger": {"period": 20, "std_dev_multiplier": 2.0},
        },
    }
    async with httpx.AsyncClient(
        base_url=RUST_API_BASE_URL,
        timeout=RUST_API_TIMEOUT_SECS,
        headers={"X-API-KEY": api_key, "Content-Type": "application/json"},
    ) as client:
        response = await client.post(INDICATORS_ENDPOINT, json=payload)
        response.raise_for_status()
        return response.json()


def _simulate_strategy(
    strategy_name:         str,
    indicators:            dict[str, Any],
    initial_capital:       float,
    position_size_percent: int,
) -> list[dict[str, Any]]:
    """
    Simulate trade execution based on strategy rules.
    Returns list of trade records with pnl.
    """
    ma20_series:  list[float] = indicators.get("ma20",  [])
    ma50_series:  list[float] = indicators.get("ma50",  [])
    rsi_series:   list[float] = [v for v in indicators.get("rsi14", [])]

    bar_count = len(ma20_series)
    trades: list[dict[str, Any]] = []
    position: float | None = None
    entry_price: float = 0.0
    capital = initial_capital

    for i in range(1, bar_count):
        ma20  = ma20_series[i]
        ma50  = ma50_series[i] if i < len(ma50_series) else ma20
        rsi   = rsi_series[i]  if i < len(rsi_series)  else 50.0

        if not (math.isfinite(ma20) and math.isfinite(ma50) and math.isfinite(rsi)):
            continue

        # Entry: MA20 crosses above MA50 and RSI not overbought
        if position is None and ma20 > ma50 and rsi < 70:
            entry_price = ma20
            position    = (capital * position_size_percent / 100) / entry_price
            continue

        # Exit: MA20 crosses below MA50 or RSI overbought
        if position is not None and (ma20 < ma50 or rsi > 70):
            exit_price = ma20
            pnl        = position * (exit_price - entry_price)
            trades.append({
                "entry_price": entry_price,
                "exit_price":  exit_price,
                "pnl":         pnl,
                "win":         pnl > 0,
            })
            capital  += pnl
            position  = None

    return trades


def _compute_metrics(trades: list[dict[str, Any]], initial_capital: float) -> dict[str, Any]:
    """Compute standard backtest performance metrics."""
    if not trades:
        return {
            "total_trades":   0,
            "winning_trades": 0,
            "losing_trades":  0,
            "win_rate":       0.0,
            "profit_factor":  0.0,
            "max_drawdown":   0.0,
            "sharpe_ratio":   0.0,
            "annual_return":  0.0,
        }

    winning = [t for t in trades if t["win"]]
    losing  = [t for t in trades if not t["win"]]
    win_rate = len(winning) / len(trades)

    gross_profit = sum(t["pnl"] for t in winning)
    gross_loss   = abs(sum(t["pnl"] for t in losing))
    profit_factor = gross_profit / gross_loss if gross_loss > 0 else gross_profit

    # Max drawdown calculation
    running_capital = initial_capital
    peak            = initial_capital
    max_drawdown    = 0.0
    for t in trades:
        running_capital += t["pnl"]
        if running_capital > peak:
            peak = running_capital
        drawdown = (peak - running_capital) / peak
        if drawdown > max_drawdown:
            max_drawdown = drawdown

    # Simplified Sharpe ratio (daily returns assumed)
    pnls = [t["pnl"] / initial_capital for t in trades]
    mean_pnl = sum(pnls) / len(pnls)
    variance = sum((p - mean_pnl) ** 2 for p in pnls) / len(pnls)
    std_dev  = math.sqrt(variance) if variance > 0 else 1.0
    sharpe   = (mean_pnl / std_dev) * math.sqrt(252) if std_dev > 0 else 0.0

    total_return  = (running_capital - initial_capital) / initial_capital
    # Annualize based on number of trades as proxy for elapsed trading days
    # Full implementation uses actual from_ms/to_ms date range
    trading_days  = max(len(trades), 1)
    years         = trading_days / 252          # 252 trading days per year
    annual_return = (1 + total_return) ** (1 / years) - 1 if years > 0 else total_return

    return {
        "total_trades":   len(trades),
        "winning_trades": len(winning),
        "losing_trades":  len(losing),
        "win_rate":       round(win_rate, 4),
        "profit_factor":  round(profit_factor, 4),
        "max_drawdown":   round(max_drawdown, 4),
        "sharpe_ratio":   round(sharpe, 4),
        "annual_return":  round(annual_return, 4),
    }