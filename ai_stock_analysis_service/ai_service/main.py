"""
AI Bridge — Python AI Service
FastAPI application entry point.
Handles SIGTERM gracefully: completes in-flight requests before shutdown.
"""
import asyncio
import logging
import os
import signal
import time
from contextlib import asynccontextmanager
from typing import Any

import uvicorn
from fastapi import FastAPI, Request, HTTPException
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field

from ai_service.constants import APP_VERSION, DEFAULT_PORT
from ai_service.models.model_registry import ModelRegistry
from ai_service.predict.handler import predict
from ai_service.backtest.engine import run_backtest
from ai_service.predict.validator import NumericValidationError

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s %(name)s — %(message)s",
)
logger = logging.getLogger(__name__)


# ── Lifespan (startup / shutdown) ─────────────────────────────────────────────

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Startup
    logger.info("AI Bridge AI Service v%s starting up", APP_VERSION)
    ModelRegistry.instance().load()
    yield
    # Shutdown: FastAPI waits for in-flight requests to complete before here
    logger.info("AI Service shutdown complete")


app = FastAPI(
    title="AI Bridge — Python AI Service",
    version=APP_VERSION,
    lifespan=lifespan,
)


# ── SIGTERM handler (complete in-flight requests) ─────────────────────────────

def _handle_sigterm(*_: Any) -> None:
    """
    On SIGTERM: FastAPI/uvicorn graceful shutdown waits for in-flight handlers.
    This handler just logs the signal receipt; uvicorn handles the rest.
    """
    logger.info("SIGTERM received — completing in-flight requests before shutdown")

signal.signal(signal.SIGTERM, _handle_sigterm)


# ── Global exception handler (error envelope format) ─────────────────────────

@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    """
    All unhandled exceptions return error envelope format.
    Python traceback is logged server-side; never exposed to Rust/frontend.
    """
    import traceback
    tb = traceback.format_exc()
    logger.error("Unhandled exception on %s: %s\n%s", request.url.path, exc, tb)

    return JSONResponse(
        status_code=500,
        content={
            "error":        "INTERNAL_AI_SERVICE_ERROR",
            "message":      str(exc),
            "traceback":    None,   # never expose traceback to caller
            "timestamp_ms": int(time.time() * 1000),
        },
    )


# ── Request / Response models ─────────────────────────────────────────────────

class PredictRequest(BaseModel):
    request_id:     str
    symbol:         str
    indicators:     dict[str, Any]
    lookback_hours: int   = Field(default=24, ge=1, le=168)
    close:          float = Field(default=0.0)


class BacktestRequest(BaseModel):
    request_id:             str
    symbol:                 str
    strategy_name:          str
    from_ms:                int
    to_ms:                  int
    initial_capital:        float = Field(gt=0)
    position_size_percent:  int   = Field(ge=1, le=100)


# ── Endpoints ─────────────────────────────────────────────────────────────────

@app.get("/health")
async def health() -> dict[str, Any]:
    registry = ModelRegistry.instance()
    return {
        "status":        "ok",
        "version":       APP_VERSION,
        "model_version": registry.model_version,
        "timestamp_ms":  int(time.time() * 1000),
    }


@app.post("/predict")
async def predict_endpoint(body: PredictRequest) -> dict[str, Any]:
    try:
        result = predict(
            symbol=body.symbol,
            indicators=body.indicators,
            lookback_hours=body.lookback_hours,
            close=body.close,
        )
        return result
    except NumericValidationError as exc:
        logger.error("Numeric validation failed for %s: %s", body.symbol, exc)
        return JSONResponse(
            status_code=422,
            content={
                "error":        "NUMERIC_VALIDATION_FAILED",
                "message":      str(exc),
                "traceback":    None,
                "timestamp_ms": int(time.time() * 1000),
            },
        )


@app.post("/backtest")
async def backtest_endpoint(body: BacktestRequest, request: Request) -> dict[str, Any]:
    api_key = request.headers.get("X-API-KEY", "")
    try:
        result = await run_backtest(
            symbol=body.symbol,
            strategy_name=body.strategy_name,
            from_ms=body.from_ms,
            to_ms=body.to_ms,
            initial_capital=body.initial_capital,
            position_size_percent=body.position_size_percent,
            request_id=body.request_id,
            api_key=api_key,
        )
        return result
    except NumericValidationError as exc:
        logger.error("Backtest metric validation failed: %s", exc)
        return JSONResponse(
            status_code=422,
            content={
                "error":        "NUMERIC_VALIDATION_FAILED",
                "message":      str(exc),
                "traceback":    None,
                "timestamp_ms": int(time.time() * 1000),
            },
        )


# ── Entry point ───────────────────────────────────────────────────────────────

if __name__ == "__main__":
    port = int(os.getenv("PORT", DEFAULT_PORT))
    uvicorn.run(
        "ai_service.main:app",
        host="0.0.0.0",
        port=port,
        log_level="info",
    )