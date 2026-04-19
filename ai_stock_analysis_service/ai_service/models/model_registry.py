"""
Model registry: load, cache, and version XGBoost models.
Falls back to rules-based logic when no trained model is available.
"""
import os
import logging
from enum import Enum
from typing import Optional
import numpy as np

from ai_service.constants import (
    MODEL_PATH,
    RULES_MODEL_VERSION,
    XGBOOST_MODEL_VERSION,
)

logger = logging.getLogger(__name__)


class ModelType(str, Enum):
    """
    Feature flag for switching between rule-based MVP and XGBoost.
    Set via MODEL_TYPE environment variable.
    """
    RULES    = "rules"
    XGBOOST  = "xgboost"


def get_active_model_type() -> ModelType:
    """Read MODEL_TYPE env var; default to rules for MVP phase."""
    raw = os.getenv("MODEL_TYPE", ModelType.RULES.value)
    try:
        return ModelType(raw)
    except ValueError:
        logger.warning("Unknown MODEL_TYPE=%s, falling back to rules", raw)
        return ModelType.RULES


class ModelRegistry:
    """
    Singleton that holds the loaded XGBoost model.
    Call load() once at startup; predict() is thread-safe.
    """

    _instance: Optional["ModelRegistry"] = None
    _model = None
    _model_version: str = RULES_MODEL_VERSION

    @classmethod
    def instance(cls) -> "ModelRegistry":
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    def load(self) -> None:
        """
        Attempt to load XGBoost model from disk.
        If MODEL_TYPE=rules or file not found, skip silently — rules logic is used.
        """
        model_type = get_active_model_type()
        if model_type == ModelType.RULES:
            logger.info("MODEL_TYPE=rules: using rules-based /predict (MVP phase)")
            return

        try:
            import xgboost as xgb
            booster = xgb.Booster()
            booster.load_model(MODEL_PATH)
            self._model = booster
            self._model_version = XGBOOST_MODEL_VERSION
            logger.info("XGBoost model loaded from %s", MODEL_PATH)
        except Exception as exc:
            logger.warning("XGBoost model load failed (%s); falling back to rules", exc)

    @property
    def model_version(self) -> str:
        return self._model_version

    def predict_proba(self, features: np.ndarray) -> tuple[float, float]:
        """
        Return (up_probability, down_probability).
        Uses XGBoost when model is loaded, otherwise rules not applicable here —
        caller should route to rules handler.
        """
        if self._model is None:
            raise RuntimeError("No XGBoost model loaded; use rules handler instead")

        import xgboost as xgb
        dmatrix = xgb.DMatrix(features.reshape(1, -1))
        raw = float(self._model.predict(dmatrix)[0])
        # XGBoost binary classifier outputs P(class=1) = P(BUY)
        up_prob   = max(0.0, min(1.0, raw))
        down_prob = 1.0 - up_prob
        return up_prob, down_prob

    @property
    def is_xgboost_ready(self) -> bool:
        return self._model is not None