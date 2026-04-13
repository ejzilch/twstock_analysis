# AI Bridge - API Contract

版本: 2.2
更新日期: 2026-04-11
設計: Gemini CLI
實作: Claude Code
狀態: 待四方簽核

---

## 基本規範

- 所有端點使用 /api/v1 前綴
- 所有 timestamp 欄位為毫秒級 UTC integer，共 13 位，例: 1704067200000
- 查詢參數時間範圍使用 from_ms / to_ms
- 所有浮點數值保證 is_finite() == true，不傳遞 Inf 或 NaN
- 所有金額數值在 i64 安全範圍內
- POST 請求包含 request_id 欄位以支援冪等性
- 認證方式: Header 攜帶 X-API-KEY: <token>，缺少或無效時回傳 401

---

## 端點目錄

| 端點 | 方法 | 用途 | 實作者 |
|------|------|------|--------|
| /api/v1/health | GET | 系統健康檢查 | Claude Code |
| /health/integrity | GET | DB/Cache 一致性 + Observability | Claude Code |
| /api/v1/symbols | GET | 支援的股票清單 | Claude Code |
| /api/v1/candles/{symbol} | GET | K 線數據與指標 | Claude Code |
| /api/v1/indicators/compute | POST | 動態參數化指標計算 | Claude Code |
| /api/v1/signals/{symbol} | GET | 交易信號 | Claude Code |
| /api/v1/predict | POST | AI 預測 (wrapper) | Claude Code |
| /api/v1/backtest | POST | 回測執行 | Claude Code (wrapper) |

---

## 端點規範

### GET /api/v1/health

```json
Response 200:
{
  "status": "ok",
  "timestamp_ms": 1704067200000,
  "components": {
    "database": "ok",
    "redis": "ok",
    "python_ai_service": "ok"
  },
  "version": "2.2.0"
}

Response 200 (部分降級):
{
  "status": "degraded",
  "timestamp_ms": 1704067200000,
  "components": {
    "database": "ok",
    "redis": "ok",
    "python_ai_service": "unavailable"
  },
  "version": "2.2.0"
}
```

---

### GET /health/integrity

EJ 每日巡視用。包含 DB/Cache 一致性檢查與系統 Observability 指標。

```json
Response 200:
{
  "status": "ok",
  "timestamp_ms": 1704067200000,
  "checks": {
    "cache_db_consistency": {
      "status": "ok",
      "sample_size": 3,
      "max_deviation_pct": 0.000
    },
    "indicator_dag_order": {
      "status": "ok",
      "last_execution_ms": 1704067190000
    },
    "python_ai_service": {
      "status": "ok",
      "last_response_ms": 45
    },
    "data_source": {
      "primary": "finmind",
      "status": "ok",
      "rate_limit_remaining_pct": 72
    }
  },
  "observability": {
    "data_latency_seconds": 87,
    "data_latency_status": "ok",
    "ai_inference_p99_ms": 145,
    "ai_inference_status": "ok",
    "api_success_rate_pct": 99.8,
    "api_success_rate_status": "ok",
    "bridge_errors_last_hour": 0
  }
}

Response 200 (降級，需 EJ 關注):
{
  "status": "degraded",
  "timestamp_ms": 1704067200000,
  "checks": {
    "cache_db_consistency": {
      "status": "mismatch",
      "sample_size": 3,
      "max_deviation_pct": 0.015,
      "note": "Exceeds 0.01% threshold"
    },
    "indicator_dag_order": { "status": "ok" },
    "python_ai_service": { "status": "ok" },
    "data_source": {
      "primary": "finmind",
      "status": "rate_limited",
      "fallback": "yfinance",
      "rate_limit_remaining_pct": 0
    }
  },
  "observability": {
    "data_latency_seconds": 720,
    "data_latency_status": "warning",
    "ai_inference_p99_ms": 380,
    "ai_inference_status": "ok",
    "api_success_rate_pct": 96.2,
    "api_success_rate_status": "warning",
    "bridge_errors_last_hour": 14
  }
}
```

Observability 欄位說明:

| 欄位 | 說明 | 告警閾值 |
|------|------|---------|
| data_latency_seconds | 最新 K 線時間與當前時間差值（秒） | > 300 秒 (5 分鐘) |
| ai_inference_p99_ms | AI 模型推論 P99 耗時（毫秒） | > 200ms |
| api_success_rate_pct | 最近 1 小時 API 請求成功率 | < 99% |
| bridge_errors_last_hour | 最近 1 小時 BridgeError 發生次數 | > 5 次 |

status 值: ok / warning / critical

---

### GET /api/v1/symbols

回傳系統目前動態管理的股票清單。

查詢參數:
- exchange (optional): TWSE / TPEX
- is_active (optional): true / false，預設只回傳 true

```json
Response 200:
{
  "symbols": [
    {
      "symbol": "2330",
      "name": "台積電",
      "exchange": "TWSE",
      "data_source": "finmind",
      "earliest_available_ms": 1388534400000,
      "latest_available_ms": 1704067200000,
      "is_active": true
    },
    {
      "symbol": "2317",
      "name": "鴻海",
      "exchange": "TWSE",
      "data_source": "finmind",
      "earliest_available_ms": 1388534400000,
      "latest_available_ms": 1704067200000,
      "is_active": true
    }
  ],
  "count": 2,
  "last_synced_ms": 1704067200000
}
```

---

### GET /api/v1/candles/{symbol}

查詢參數:
- from_ms (required): 開始時間戳，毫秒
- to_ms (required): 結束時間戳，毫秒
- interval (optional): 1m / 5m / 15m / 1h / 4h / 1d，預設 1h
- indicators (optional): 逗號分隔，如 ma20,rsi,macd
- cursor (optional): 分頁游標，由上一頁 response 提供

數量限制:
- 單次請求最多回傳 2000 根 K 線
- 超過範圍回傳 400 QUERY_RANGE_TOO_LARGE，不自動截斷
- 使用 next_cursor 分頁取得後續資料

```json
Response 200:
{
  "symbol": "2330",
  "interval": "1h",
  "from_ms": 1704067200000,
  "to_ms": 1704153600000,
  "candles": [
    {
      "timestamp_ms": 1704067200000,
      "open": 150.0,
      "high": 151.5,
      "low": 149.5,
      "close": 151.0,
      "volume": 1000000,
      "indicators": {
        "ma20": 150.5,
        "rsi": 55.2,
        "macd": {
          "macd_line": 0.5,
          "signal_line": 0.3,
          "histogram": 0.2
        }
      }
    }
  ],
  "count": 168,
  "total_available": 2500,
  "next_cursor": "cursor_abc123",
  "source": "database",
  "cached": false,
  "computed_at_ms": 1704067200000
}
```

next_cursor 為 null 表示已取完全部資料。

---

### POST /api/v1/indicators/compute

```json
Request:
{
  "request_id": "req-20260411-001",
  "symbol": "2330",
  "from_ms": 1704067200000,
  "to_ms": 1704153600000,
  "interval": "1h",
  "indicators": {
    "ma": [5, 10, 20, 50, 200],
    "rsi": [14],
    "macd": [12, 26, 9],
    "bollinger": {
      "period": 20,
      "std_dev_multiplier": 2.0
    }
  }
}

Response 200:
{
  "symbol": "2330",
  "interval": "1h",
  "from_ms": 1704067200000,
  "to_ms": 1704153600000,
  "indicators": {
    "ma5":  [150.0, 150.1],
    "ma20": [150.2, 150.3],
    "rsi14": [55.0, 56.2],
    "macd": {
      "macd_line": [0.1, 0.15],
      "signal_line": [0.08, 0.12],
      "histogram": [0.02, 0.03]
    },
    "bollinger": {
      "upper":  [152.5, 152.7],
      "middle": [150.5, 150.6],
      "lower":  [148.5, 148.5]
    }
  },
  "computed_at_ms": 1704067200000,
  "computation_time_ms": 45,
  "cached": false,
  "dag_execution_order": ["ma5", "ma20", "rsi14", "macd", "bollinger"]
}
```

---

### GET /api/v1/signals/{symbol}

查詢參數: from_ms (required), to_ms (required)

```json
Response 200:
{
  "symbol": "2330",
  "from_ms": 1704067200000,
  "to_ms": 1704153600000,
  "signals": [
    {
      "id": "sig-20260411-001",
      "timestamp_ms": 1704067200000,
      "signal_type": "BUY",
      "confidence": 0.75,
      "entry_price": 150.5,
      "target_price": 155.0,
      "stop_loss": 148.0,
      "reason": "MA20 > MA50, RSI < 30, AI confidence 0.75",
      "source": "ai_ensemble",
      "reliability": "high",
      "fallback_reason": null
    },
    {
      "id": "sig-20260411-002",
      "timestamp_ms": 1704070800000,
      "signal_type": "SELL",
      "confidence": 0.52,
      "entry_price": 152.0,
      "target_price": 148.0,
      "stop_loss": 153.5,
      "reason": "MACD histogram turning negative",
      "source": "technical_only",
      "reliability": "low",
      "fallback_reason": "AI_SERVICE_TIMEOUT"
    }
  ],
  "count": 2
}
```

signal source 說明:
- ai_ensemble: Python AI 模型集成預測，正常狀態
- technical_only: AI 不可用，降級至技術指標
- manual_override: 人工干預信號

reliability 說明:
- high: AI 正常，confidence > 0.7
- medium: AI 正常，confidence 0.5 ~ 0.7
- low: AI 超時降級，或技術指標強度弱
- unknown: 無法取得任何信號

---

### POST /api/v1/predict

```json
Request:
{
  "request_id": "req-20260411-002",
  "symbol": "2330",
  "indicators": {
    "ma20": 150.5,
    "ma50": 149.8,
    "rsi": 55.2,
    "macd_line": 0.5
  },
  "lookback_hours": 24
}

Response 200:
{
  "symbol": "2330",
  "up_probability": 0.72,
  "down_probability": 0.28,
  "confidence_score": 0.85,
  "model_version": "xgboost_v2.1",
  "inference_time_ms": 12,
  "computed_at_ms": 1704067200000
}
```

Python 端保證: up_probability + down_probability == 1.0，所有數值 is_finite() == true

---

### POST /api/v1/backtest

```json
Request:
{
  "request_id": "req-20260411-003",
  "symbol": "2330",
  "strategy_name": "trend_follow_v1",
  "from_ms": 1704067200000,
  "to_ms": 1735689600000,
  "initial_capital": 10000.0,
  "position_size_percent": 100
}

Response 200:
{
  "backtest_id": "bt-20260411-001",
  "symbol": "2330",
  "strategy_name": "trend_follow_v1",
  "from_ms": 1704067200000,
  "to_ms": 1735689600000,
  "initial_capital": 10000.0,
  "final_capital": 11500.0,
  "metrics": {
    "total_trades": 10,
    "winning_trades": 6,
    "losing_trades": 4,
    "win_rate": 0.60,
    "profit_factor": 2.1,
    "max_drawdown": 0.12,
    "sharpe_ratio": 1.45,
    "annual_return": 0.15
  },
  "created_at_ms": 1704067200000
}
```

---

## 全域錯誤處理

### HTTP Status 對應

| Status | 情境 |
|--------|------|
| 200 | 成功 |
| 400 | 參數錯誤 |
| 401 | 認證失敗 (X-API-KEY 缺少或無效) |
| 403 | 權限不足 |
| 404 | 資源不存在 |
| 422 | 數值範圍超出安全限制 |
| 500 | 服務器內部錯誤 |
| 503 | 服務暫時不可用 |
| 504 | 依賴服務超時 |

### 認證錯誤格式

```json
Response 401:
{
  "error_code": "UNAUTHORIZED",
  "message": "Missing or invalid X-API-KEY header.",
  "fallback_available": false,
  "timestamp_ms": 1704067200000,
  "request_id": null
}
```

### 一般錯誤響應格式

```json
{
  "error_code": "AI_SERVICE_TIMEOUT",
  "message": "Python AI service did not respond within 10s. Falling back to technical indicators.",
  "fallback_available": true,
  "timestamp_ms": 1704067200000,
  "request_id": "req-20260411-001"
}
```

### 錯誤碼清單

| error_code | HTTP Status | 前端顯示建議 |
|-----------|-------------|------------|
| UNAUTHORIZED | 401 | 請確認 API 金鑰是否正確 |
| AI_SERVICE_TIMEOUT | 504 | AI 算力繁忙，使用技術指標信號 |
| AI_SERVICE_UNAVAILABLE | 503 | AI 服務暫停，請稍後 |
| DATA_SOURCE_INTERRUPTED | 503 | 數據源暫中斷，顯示快取數據 |
| DATA_SOURCE_RATE_LIMITED | 503 | 資料來源達到請求上限，系統切換備援中 |
| INDICATOR_COMPUTE_FAILED | 500 | 指標計算異常，請重新整理 |
| CACHE_MISS_FALLBACK | 200 | 靜默，前端不顯示 |
| COMPUTATION_OVERFLOW | 422 | 計算數值異常，請聯繫支援 |
| INVALID_INDICATOR_CONFIG | 400 | 顯示具體錯誤訊息 |
| SYMBOL_NOT_FOUND | 404 | 找不到該股票 |
| QUERY_RANGE_TOO_LARGE | 400 | 查詢範圍過大，請縮小時間區間或使用分頁 |

### 超時與降級策略

| 依賴服務 | Timeout | 降級策略 |
|---------|---------|---------|
| Python AI Service | 10s | 使用技術指標信號，source = technical_only |
| PostgreSQL | 5s | 回傳 503 DATA_SOURCE_INTERRUPTED |
| Redis | 1s | 直接計算，回傳 200 CACHE_MISS_FALLBACK |
| FinMind API | 15s | 切換 yfinance，回傳 503 DATA_SOURCE_RATE_LIMITED |

---

## Breaking Change 規範

需要版本升級 (v1 -> v2):
- 移除現有端點或欄位
- 改變欄位型別或語義
- 改變 timestamp 格式

Non-breaking (可在同版本更新):
- 新增端點或可選欄位
- 新增 error_code
- 改善錯誤訊息

升級流程:
1. Gemini 提案新 Spec
2. EJ 簽核
3. Claude Code 實作新舊版本並行
4. Codex 遷移前端
5. 舊版 3 個月後棄用

---

## 簽核

| 角色 | 狀態 | 日期 |
|------|------|------|
| EJ (PM) | 已簽核 | 2026-04-13 |
| Gemini CLI | 待簽核 | |
| Claude Code | 待簽核 | |
| OpenAI Codex | 待簽核 | |

版本: 2.2
下次審查: 2026-04-25
