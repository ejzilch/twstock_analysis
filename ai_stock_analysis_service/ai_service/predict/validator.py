"""
Numeric safety validation before returning values to Rust.
All outputs must pass is_finite() and i64 range checks.
"""
import math
from typing import Any
from ai_service.constants import I64_MAX, I64_MIN


class NumericValidationError(Exception):
    """Raised when output values fail safety checks."""


def assert_finite(value: float, field_name: str) -> None:
    """Raise NumericValidationError if value is NaN or Inf."""
    if not math.isfinite(value):
        raise NumericValidationError(
            f"Field '{field_name}' is not finite: {value}"
        )


def assert_in_i64_range(value: int, field_name: str) -> None:
    """Raise NumericValidationError if integer exceeds i64 safe range."""
    if not (I64_MIN <= value <= I64_MAX):
        raise NumericValidationError(
            f"Field '{field_name}' is out of i64 range: {value}"
        )


def validate_prediction_output(payload: dict[str, Any]) -> None:
    """
    Validate all numeric fields in a /predict response.
    Enforces:
      - up_probability + down_probability == 1.0
      - All float fields are finite
    """
    for field in ("up_probability", "down_probability", "confidence_score"):
        assert_finite(payload[field], field)

    total = payload["up_probability"] + payload["down_probability"]
    if not math.isclose(total, 1.0, rel_tol=1e-9):
        raise NumericValidationError(
            f"up_probability + down_probability must equal 1.0, got {total}"
        )


def validate_backtest_metrics(metrics: dict[str, Any]) -> None:
    """Validate all float fields in backtest metrics."""
    float_fields = (
        "win_rate", "profit_factor", "max_drawdown",
        "sharpe_ratio", "annual_return",
    )
    for field in float_fields:
        assert_finite(metrics[field], field)