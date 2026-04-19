"""
All business constants for AI Bridge Python AI Service.
No hardcoded numeric values are allowed elsewhere — reference this file.
"""

# ── Prediction ────────────────────────────────────────────────────────────────

# Label definition thresholds (confirmed by EJ 2026-04-16)
LABEL_BUY_THRESHOLD:  float = 0.015   # +1.5% return rate
LABEL_SELL_THRESHOLD: float = -0.015  # -1.5% return rate
PREDICTION_WINDOW:    int   = 6       # future K-line bars (15m each = 1.5 hours)

# Confidence bounds
MIN_CONFIDENCE: float = 0.0
MAX_CONFIDENCE: float = 1.0

# Probability sum must equal exactly 1.0
PROBABILITY_SUM: float = 1.0

# ── Rules-based model (MVP phase) ─────────────────────────────────────────────

RULES_MODEL_VERSION: str   = "rules_v1.0"
RULES_CONFIDENCE:    float = 0.5    # fixed mid-confidence for rule-based signals

# RSI thresholds
RSI_OVERSOLD:  float = 30.0
RSI_OVERBOUGHT: float = 70.0

# RSI scoring weights
WEIGHT_RSI:       int = 2
WEIGHT_MA_CROSS:  int = 1
WEIGHT_MACD:      int = 1
WEIGHT_BOLLINGER: int = 1

# Max possible score (sum of all positive weights)
MAX_SCORE: int = WEIGHT_RSI + WEIGHT_MA_CROSS + WEIGHT_MACD + WEIGHT_BOLLINGER

# ── XGBoost model ─────────────────────────────────────────────────────────────

XGBOOST_MODEL_VERSION: str   = "xgboost_v1.0"
XGBOOST_MIN_ACCURACY:  float = 0.55   # minimum test-set accuracy before deployment
MODEL_PATH: str = "ai_service/models/xgboost_model.json"

# ── Numeric safety limits ─────────────────────────────────────────────────────

# i64 safe range (Rust constraint)
I64_MAX:  int = 9_223_372_036_854_775_807
I64_MIN:  int = -9_223_372_036_854_775_808

# Near-limit price movement (Taiwan daily limit ±10%)
NEAR_LIMIT_UP_THRESHOLD:   float = 0.09   # >9% — exclude from training
NEAR_LIMIT_DOWN_THRESHOLD: float = -0.09  # <-9% — exclude from training

# ── API Client (Rust calls) ───────────────────────────────────────────────────

RUST_API_BASE_URL:       str = "http://localhost:8089"
RUST_API_TIMEOUT_SECS:   int = 30
INDICATORS_ENDPOINT:     str = "/api/v1/indicators/compute"

# Serialization thresholds (must mirror Rust constants)
MSGPACK_CANDLE_THRESHOLD: int = 1000   # use MsgPack when candle_count > this

# ── Service ───────────────────────────────────────────────────────────────────

APP_VERSION:    str = "0.1.0"
DEFAULT_PORT:   int = 8001
SIGTERM_TIMEOUT_SECS: int = 30   # max seconds to wait for in-flight requests