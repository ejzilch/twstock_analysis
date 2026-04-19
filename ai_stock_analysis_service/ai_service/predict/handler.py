"""
/predict endpoint handler.
Routes to rules-based MVP logic or XGBoost based on ModelRegistry state.
"""
import time
import logging
import math
from typing import Any

from ai_service.constants import (
    RSI_OVERSOLD, RSI_OVERBOUGHT,
    WEIGHT_RSI, WEIGHT_MA_CROSS, WEIGHT_MACD, WEIGHT_BOLLINGER, MAX_SCORE,
    RULES_MODEL_VERSION, RULES_CONFIDENCE,
)
from ai_service.predict.features import extract_features
from ai_service.predict.validator import validate_prediction_output
from ai_service.models.model_registry import ModelRegistry

logger = logging.getLogger(__name__)


def predict(
    symbol: str,
    indicators: dict[str, Any],
    lookback_hours: int,
    close: float = 0.0,
) -> dict[str, Any]:
    """
    Main prediction entry point.
    Selects rules or XGBoost automatically based on model availability.

    Args:
        symbol:         Stock symbol (for logging).
        indicators:     Dict of indicator values from Rust API.
        lookback_hours: Context window (passed through to response).
        close:          Current close price for feature engineering.

    Returns:
        Validated prediction dict ready to serialize as JSON response.
    """
    start_ms = time.time() * 1000
    registry = ModelRegistry.instance()

    if registry.is_xgboost_ready:
        result = _predict_xgboost(symbol, indicators, close, registry)
    else:
        result = _predict_rules(symbol, indicators)

    result["inference_time_ms"] = round(time.time() * 1000 - start_ms)
    result["computed_at_ms"]    = int(time.time() * 1000)

    validate_prediction_output(result)
    return result


# ── Rules-based MVP ───────────────────────────────────────────────────────────

def _predict_rules(symbol: str, indicators: dict[str, Any]) -> dict[str, Any]:
    """
    Scoring model using technical indicator rules.
    Score range: -MAX_SCORE to +MAX_SCORE.
    Mapped linearly to up_probability in [0, 1].
    """
    score = 0

    rsi = float(indicators.get("rsi", 50.0))
    if rsi < RSI_OVERSOLD:
        score += WEIGHT_RSI
    elif rsi > RSI_OVERBOUGHT:
        score -= WEIGHT_RSI

    ma20 = float(indicators.get("ma20", 0.0))
    ma50 = float(indicators.get("ma50", 0.0))
    if ma20 > 0 and ma50 > 0:
        if ma20 > ma50:
            score += WEIGHT_MA_CROSS
        else:
            score -= WEIGHT_MA_CROSS

    macd_raw = indicators.get("macd", {})
    if isinstance(macd_raw, dict):
        histogram = float(macd_raw.get("histogram", 0.0))
        if histogram > 0:
            score += WEIGHT_MACD
        elif histogram < 0:
            score -= WEIGHT_MACD

    boll_raw = indicators.get("bollinger", {})
    if isinstance(boll_raw, dict):
        close = float(indicators.get("close", 0.0))
        upper = float(boll_raw.get("upper", 0.0))
        lower = float(boll_raw.get("lower", 0.0))
        if close > 0 and upper > 0 and lower > 0:
            if close < lower:
                score += WEIGHT_BOLLINGER
            elif close > upper:
                score -= WEIGHT_BOLLINGER

    up_prob   = (score + MAX_SCORE) / (2 * MAX_SCORE)
    up_prob   = max(0.0, min(1.0, up_prob))
    down_prob = 1.0 - up_prob

    logger.debug("rules predict symbol=%s score=%d up=%.3f", symbol, score, up_prob)

    return {
        "symbol":           symbol,
        "up_probability":   up_prob,
        "down_probability": down_prob,
        "confidence_score": RULES_CONFIDENCE,
        "model_version":    RULES_MODEL_VERSION,
    }


# ── XGBoost ───────────────────────────────────────────────────────────────────

def _predict_xgboost(
    symbol: str,
    indicators: dict[str, Any],
    close: float,
    registry: ModelRegistry,
) -> dict[str, Any]:
    """XGBoost inference path (active after 2026-06 model training)."""
    features = extract_features(indicators, close)
    up_prob, down_prob = registry.predict_proba(features)
    confidence = abs(up_prob - 0.5) * 2   # distance from 50/50 → [0, 1]

    logger.debug("xgb predict symbol=%s up=%.3f conf=%.3f", symbol, up_prob, confidence)

    return {
        "symbol":           symbol,
        "up_probability":   up_prob,
        "down_probability": down_prob,
        "confidence_score": confidence,
        "model_version":    registry.model_version,
    }