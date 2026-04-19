"""
Test suite for AI Bridge Python AI Service.
Run: pytest tests/ -v
"""
import math
import pytest
from unittest.mock import patch, MagicMock

from ai_service.predict.handler import predict, _predict_rules
from ai_service.predict.features import extract_features, FEATURE_NAMES
from ai_service.predict.validator import (
    validate_prediction_output,
    validate_backtest_metrics,
    NumericValidationError,
)
from ai_service.serialization import decode_request, encode_response
from ai_service.constants import (
    RULES_MODEL_VERSION,
    RULES_CONFIDENCE,
    MSGPACK_CANDLE_THRESHOLD,
)


# ── Sample fixtures ───────────────────────────────────────────────────────────

SAMPLE_INDICATORS = {
    "ma5":  150.0,
    "ma20": 149.5,
    "ma50": 148.0,
    "rsi":  45.0,
    "macd": {
        "macd_line":   0.5,
        "signal_line": 0.3,
        "histogram":   0.2,
    },
    "bollinger": {
        "upper":  152.0,
        "middle": 149.5,
        "lower":  147.0,
    },
    "close": 150.5,
}


# ── Rules-based predict ───────────────────────────────────────────────────────

class TestRulesPredict:

    def test_output_keys_present(self):
        result = _predict_rules("2330", SAMPLE_INDICATORS)
        assert "up_probability"   in result
        assert "down_probability" in result
        assert "confidence_score" in result
        assert "model_version"    in result

    def test_probabilities_sum_to_one(self):
        result = _predict_rules("2330", SAMPLE_INDICATORS)
        total = result["up_probability"] + result["down_probability"]
        assert math.isclose(total, 1.0, rel_tol=1e-9)

    def test_model_version_is_rules(self):
        result = _predict_rules("2330", SAMPLE_INDICATORS)
        assert result["model_version"] == RULES_MODEL_VERSION

    def test_confidence_is_fixed(self):
        result = _predict_rules("2330", SAMPLE_INDICATORS)
        assert result["confidence_score"] == RULES_CONFIDENCE

    def test_oversold_rsi_biases_up(self):
        indicators = {**SAMPLE_INDICATORS, "rsi": 20.0}  # oversold
        result = _predict_rules("2330", indicators)
        assert result["up_probability"] > 0.5

    def test_overbought_rsi_biases_down(self):
        indicators = {**SAMPLE_INDICATORS, "rsi": 80.0}  # overbought
        result = _predict_rules("2330", indicators)
        assert result["down_probability"] > 0.5

    def test_probabilities_within_bounds(self):
        result = _predict_rules("2330", SAMPLE_INDICATORS)
        assert 0.0 <= result["up_probability"]   <= 1.0
        assert 0.0 <= result["down_probability"] <= 1.0

    def test_all_values_finite(self):
        result = _predict_rules("2330", SAMPLE_INDICATORS)
        assert math.isfinite(result["up_probability"])
        assert math.isfinite(result["down_probability"])
        assert math.isfinite(result["confidence_score"])


# ── Feature engineering ───────────────────────────────────────────────────────

class TestFeatureEngineering:

    def test_output_length_matches_feature_names(self):
        features = extract_features(SAMPLE_INDICATORS, close=150.5)
        assert len(features) == len(FEATURE_NAMES)

    def test_all_features_finite(self):
        features = extract_features(SAMPLE_INDICATORS, close=150.5)
        assert all(math.isfinite(f) for f in features)

    def test_missing_indicators_handled(self):
        """Should not raise even with minimal indicator data."""
        minimal = {"rsi": 50.0}
        features = extract_features(minimal, close=100.0)
        assert len(features) == len(FEATURE_NAMES)
        assert all(math.isfinite(f) for f in features)


# ── Validator ─────────────────────────────────────────────────────────────────

class TestValidator:

    def test_valid_prediction_passes(self):
        payload = {
            "up_probability":   0.7,
            "down_probability": 0.3,
            "confidence_score": 0.8,
        }
        validate_prediction_output(payload)  # should not raise

    def test_nan_raises(self):
        payload = {
            "up_probability":   float("nan"),
            "down_probability": 0.3,
            "confidence_score": 0.5,
        }
        with pytest.raises(NumericValidationError):
            validate_prediction_output(payload)

    def test_inf_raises(self):
        payload = {
            "up_probability":   float("inf"),
            "down_probability": 0.0,
            "confidence_score": 0.5,
        }
        with pytest.raises(NumericValidationError):
            validate_prediction_output(payload)

    def test_probabilities_not_summing_to_one_raises(self):
        payload = {
            "up_probability":   0.6,
            "down_probability": 0.6,   # sum = 1.2
            "confidence_score": 0.5,
        }
        with pytest.raises(NumericValidationError):
            validate_prediction_output(payload)

    def test_valid_backtest_metrics_pass(self):
        metrics = {
            "win_rate": 0.6, "profit_factor": 1.5,
            "max_drawdown": 0.1, "sharpe_ratio": 1.2, "annual_return": 0.15,
        }
        validate_backtest_metrics(metrics)  # should not raise


# ── Serialization ─────────────────────────────────────────────────────────────

class TestSerialization:

    def test_json_decode(self):
        import json
        body = json.dumps({"key": "value"}).encode()
        result = decode_request("application/json", body)
        assert result == {"key": "value"}

    def test_msgpack_decode(self):
        import msgpack
        body = msgpack.packb({"key": "value"}, use_bin_type=True)
        result = decode_request("application/msgpack", body)
        assert result == {"key": "value"}

    def test_small_candle_count_uses_json(self):
        _, content_type = encode_response({"data": []}, candle_count=100)
        assert content_type == "application/json"

    def test_large_candle_count_uses_msgpack(self):
        _, content_type = encode_response({"data": []}, candle_count=MSGPACK_CANDLE_THRESHOLD + 1)
        assert content_type == "application/msgpack"