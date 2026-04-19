"""
Feature engineering: transform raw indicator values into model input vector.
All features derived from Rust-computed indicators — never self-computed.
"""
import numpy as np
from typing import Any


# Feature names in exact order expected by XGBoost model.
# Must match training pipeline column order.
FEATURE_NAMES: list[str] = [
    "ma5",
    "ma20",
    "ma50",
    "close_to_ma20_ratio",    # close / ma20
    "rsi",
    "macd_line",
    "signal_line",
    "macd_histogram",
    "bollinger_upper",
    "bollinger_lower",
    "bollinger_width",        # (upper - lower) / middle
    "close_to_boll_position", # (close - lower) / (upper - lower)
]


def extract_features(indicators: dict[str, Any], close: float) -> np.ndarray:
    """
    Build feature vector from indicator dict and current close price.
    Missing indicators are filled with 0.0 (model trained to handle sparse input).

    Args:
        indicators: Indicator values from /api/v1/indicators/compute response.
        close:      Current bar close price.

    Returns:
        1-D numpy array of shape (len(FEATURE_NAMES),).
    """
    ma5   = float(indicators.get("ma5",  0.0))
    ma20  = float(indicators.get("ma20", 0.0))
    ma50  = float(indicators.get("ma50", 0.0))
    rsi   = float(indicators.get("rsi",  50.0))

    macd_raw     = indicators.get("macd", {})
    macd_line    = float(macd_raw.get("macd_line",   0.0)) if isinstance(macd_raw, dict) else 0.0
    signal_line  = float(macd_raw.get("signal_line", 0.0)) if isinstance(macd_raw, dict) else 0.0
    histogram    = float(macd_raw.get("histogram",   0.0)) if isinstance(macd_raw, dict) else 0.0

    boll_raw    = indicators.get("bollinger", {})
    boll_upper  = float(boll_raw.get("upper",  close)) if isinstance(boll_raw, dict) else close
    boll_lower  = float(boll_raw.get("lower",  close)) if isinstance(boll_raw, dict) else close
    boll_middle = float(boll_raw.get("middle", close)) if isinstance(boll_raw, dict) else close

    close_to_ma20    = close / ma20 if ma20 != 0 else 1.0
    boll_width       = (boll_upper - boll_lower) / boll_middle if boll_middle != 0 else 0.0
    boll_band_range  = boll_upper - boll_lower
    close_boll_pos   = (close - boll_lower) / boll_band_range if boll_band_range != 0 else 0.5

    return np.array([
        ma5, ma20, ma50,
        close_to_ma20,
        rsi,
        macd_line, signal_line, histogram,
        boll_upper, boll_lower, boll_width,
        close_boll_pos,
    ], dtype=np.float64)