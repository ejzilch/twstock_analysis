# AI Bridge - 系統架構設計

版本: 2.2
更新日期: 2026-04-11
維護者: EJ (PM)

---

## 系統架構

```
Frontend (Next.js + TradingView)
  |
  | HTTP (REST)
  v
Rust API Gateway (Axum)
  |-- src/api/     路由、中介軟體、錯誤處理
  |-- src/core/    技術指標計算 (MA, RSI, MACD, Bollinger)
  |-- src/data/    外部 API 擷取、DB 寫入、快取失效
  |
  |-- PostgreSQL   歷史 K 線、信號、回測結果、Symbol 清單
  |-- Redis        指標快取 (TTL 管理)
  |
  | HTTP (內部，10s timeout)
  | 序列化格式: JSON (預設) / MsgPack (大批量傳輸，> 1000 根 K 線)
  v
Python AI Service (FastAPI)
  |-- ai_service/  模型推論、特徵工程
  |-- 回測引擎 (透過 Rust API 取指標)
```

---

## Rust 模組邊界

```
src/
  data/           Gemini 負責
    mod.rs
    fetch.rs      外部股票 API 呼叫 (FinMind 主力 / yfinance 備用)
    fetch_rate_limiter.rs   FinMind API 排程限流，預留付費升級介面
    db.rs         DB 寫入 (Bulk Insert) + 快取失效觸發
    symbol_sync.rs          動態 Symbol 清單同步邏輯
    models.rs     RawCandle struct (含 DataSource)

  models/         Gemini 負責
    mod.rs
    candle.rs     Candle struct，RawCandle -> Candle 轉換
    indicators.rs Indicators struct，Signal struct
    symbol.rs     Symbol struct，SymbolMeta struct

  core/           Claude Code 負責
    mod.rs        BridgeError 統一錯誤列舉定義
    indicators/
      factory.rs  指標工廠，拓撲排序計算順序
      ma.rs
      rsi.rs
      macd.rs
      bollinger.rs
    strategy/
      signal_aggregator.rs  信號產生，透過 channel 通知 (預留 WebSocket 擴充)
      traits.rs

  api/            Claude Code 負責
    handlers/
      candles.rs
      indicators.rs
      signals.rs
      health.rs
      symbols.rs            GET /api/v1/symbols 端點
    middleware/
      auth.rs
      rate_limit.rs
      error.rs
    models/
      request.rs
      response.rs   包含 SignalSource, ReliabilityLevel enum

  ai_client/      Claude Code 負責
    client.rs     Python AI Service HTTP client，含 timeout、fallback、BridgeError 轉換
    serialization.rs        JSON / MsgPack 格式自動選擇

  main.rs         Claude Code 負責
                  Graceful Shutdown: SIGINT / SIGTERM 處理
```

---

## 資料結構定義

```rust
// src/data/models.rs (Gemini)
pub struct RawCandle {
    pub symbol: String,
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub source: DataSource,
}

pub enum DataSource {
    FinMind,   // 主力：台股 (TWSE/TPEX)
    YFinance,  // 備用：補歷史資料，不放即時路徑
}

// src/models/candle.rs (Gemini)
// 內部 domain model，供 IndicatorFactory 與 SignalAggregator 使用
// 不直接序列化為 API response，對外格式由 src/api/models/response.rs 負責
pub struct Candle {
    pub symbol:       String,
    pub interval:     String,
    pub timestamp_ms: i64,
    pub open:         f64,
    pub high:         f64,
    pub low:          f64,
    pub close:        f64,
    pub volume:       u64,     // 不可為負，對齊 RawCandle
    pub indicators:   HashMap<String, IndicatorValue>,
}

// src/models/symbol.rs (Gemini)
pub struct SymbolMeta {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    pub data_source: DataSource,
    pub earliest_available_ms: i64,
    pub latest_available_ms: i64,
    pub is_active: bool,
}

// src/models/indicators.rs (Gemini)
// 指標值的允許形態，限制只能是純量或 MACD 結構，禁止 serde_json::Value
#[serde(untagged)]
pub enum IndicatorValue {
    Scalar(f64),        // MA, RSI, Bollinger 單一數值
    Macd(MacdValue),    // MACD 三線結構
}

// MACD 指標結構，對應 API_CONTRACT.md 的 macd response 欄位
pub struct MacdValue {
    pub macd_line:   f64,
    pub signal_line: f64,
    pub histogram:   f64,
}

// src/api/models/response.rs (Claude Code)
// 對外 API 的 K 線單筆資料，從 Candle domain model 轉換而來
pub struct CandleResponse {
    pub timestamp_ms: i64,
    pub open:         f64,
    pub high:         f64,
    pub low:          f64,
    pub close:        f64,
    pub volume:       u64,
    pub indicators:   HashMap<String, IndicatorValue>,
}

// 對外 API 的完整 candles response，含分頁與 meta 資訊
// 對應 API_CONTRACT.md GET /api/v1/candles/{symbol} response 格式
pub struct CandlesApiResponse {
    pub symbol:          String,
    pub interval:        String,
    pub from_ms:         i64,
    pub to_ms:           i64,
    pub candles:         Vec<CandleResponse>,
    pub count:           usize,
    pub total_available: usize,
    pub next_cursor:     Option<String>,
    pub source:          String,
    pub cached:          bool,
    pub computed_at_ms:  i64,
}

// Candle -> CandleResponse 轉換，在 handler 層執行
// domain model 與 response struct 完全解耦，API 格式變動不影響核心計算
impl From<Candle> for CandleResponse {
    fn from(candle: Candle) -> Self {
        Self {
            timestamp_ms: candle.timestamp_ms,
            open:         candle.open,
            high:         candle.high,
            low:          candle.low,
            close:        candle.close,
            volume:       candle.volume,
            indicators:   candle.indicators,
        }
    }
}

// src/api/models/response.rs (Claude Code)
pub struct TradeSignalResponse {
    pub id: String,
    pub symbol: String,
    pub signal_type: SignalType,
    pub confidence: f64,
    pub source: SignalSource,
    pub reliability: ReliabilityLevel,
    pub fallback_reason: Option<String>,
    pub entry_price: f64,
    pub target_price: f64,
    pub stop_loss: f64,
    pub reason: String,
    pub timestamp_ms: i64,
}

pub enum SignalSource {
    AiEnsemble,
    TechnicalOnly,
    ManualOverride,
}

pub enum ReliabilityLevel {
    High,
    Medium,
    Low,
    Unknown,
}
```

---

## BridgeError 統一錯誤橋接設計

Rust 與 Python 之間通訊失敗時，透過 BridgeError 統一分類，確保錯誤資訊完整記錄於 tracing log，不對外暴露內部細節，前端只看到對應的降級行為。

```rust
// src/core/mod.rs (Claude Code)
// Rust <-> Python 通訊錯誤的統一分類
// 每個 variant 攜帶足夠的上下文供除錯，不對前端暴露堆疊

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    // Python 服務主動回傳非 2xx，攜帶 HTTP status 與 body
    #[error("Python service returned error: status={status_code}, body={response_body}")]
    PythonServiceError {
        status_code: u16,
        response_body: String,
    },

    // Python 程序崩潰或 OOM，連線被強制關閉
    #[error("Python service connection lost: {reason}")]
    PythonConnectionLost {
        reason: String,
    },

    // 請求超過 10 秒未回應
    #[error("Python service timed out after {timeout_secs}s for symbol={symbol}")]
    PythonTimeout {
        timeout_secs: u64,
        symbol: String,
    },

    // Python 回傳格式不符合預期 schema
    #[error("Python response deserialization failed: {detail}")]
    PythonResponseMalformed {
        detail: String,
        raw_response: String,  // 寫入 tracing log
    },

    // Python 端明確回傳錯誤訊息與堆疊 (透過約定的 error envelope 傳遞)
    #[error("Python reported internal error: {message}")]
    PythonInternalError {
        message: String,
        traceback: Option<String>,  // 寫入 tracing log，不對前端暴露
    },
}
```

Python 端錯誤回傳格式（error envelope 約定）：

```json
{
  "error": "MODEL_INFERENCE_FAILED",
  "message": "XGBoost prediction failed due to NaN in feature matrix",
  "traceback": "Traceback (most recent call last):\n  File ...",
  "timestamp_ms": 1704067200000
}
```

Rust 側對應處理邏輯：

```rust
// src/ai_client/client.rs (Claude Code)
// Python traceback 完整寫入 tracing log，對前端只回傳降級行為

match bridge_error {
    BridgeError::PythonInternalError { message, traceback } => {
        tracing::error!(
            symbol = symbol,
            python_error = %message,
            python_traceback = traceback.as_deref().unwrap_or("none"),
            "Python internal error captured"
        );
        build_technical_fallback_response(&indicators, message)
    }
    BridgeError::PythonConnectionLost { reason } => {
        tracing::error!(
            symbol = symbol,
            reason = %reason,
            "Python process likely crashed or OOM"
        );
        build_technical_fallback_response(&indicators, "AI_SERVICE_UNAVAILABLE".to_string())
    }
    BridgeError::PythonTimeout { timeout_secs, symbol } => {
        tracing::warn!(
            symbol = %symbol,
            timeout_secs = timeout_secs,
            "Python service timed out, falling back to technical signal"
        );
        build_technical_fallback_response(&indicators, "AI_SERVICE_TIMEOUT".to_string())
    }
    // 其他 variant 類推
}
```

---

## 序列化格式策略

Rust 與 Python 之間的資料交換格式依傳輸量選擇，不強制單一格式。

| 場景 | 格式 | 理由 |
|------|------|------|
| 一般推論請求 (/predict) | JSON | 資料量小，維護性優先 |
| 大批量 K 線傳輸 (> 1000 根) | MsgPack | 序列化速度與體積優於 JSON |
| 回測指標批次傳輸 | MsgPack | Python json 模組大量浮點數下成為瓶頸 |
| 對外 REST API | JSON | 外部介面固定 JSON，不受影響 |

實作規範：

```rust
// src/ai_client/serialization.rs (Claude Code)
// 依 payload 大小自動選擇序列化格式
// Content-Type: application/json    -> JSON
// Content-Type: application/msgpack -> MsgPack

pub enum SerializationFormat {
    Json,
    MsgPack,
}

impl SerializationFormat {
    pub fn select_by_candle_count(candle_count: usize) -> Self {
        if candle_count > 1000 {
            SerializationFormat::MsgPack
        } else {
            SerializationFormat::Json
        }
    }
}
```

Python 端對應：

```python
# ai_service/serialization.py (Codex 負責)
# 依 Content-Type header 自動解析

import msgpack
import json

def decode_request(content_type: str, body: bytes) -> dict:
    if content_type == "application/msgpack":
        return msgpack.unpackb(body, raw=False)
    return json.loads(body)
```

注意事項：
- MsgPack 僅限 Rust <-> Python 內部傳輸，對外 API 永遠使用 JSON
- Python 端需引入 `msgpack` 套件，列入 requirements.txt
- Arrow 格式保留為未來擴充選項，MVP 不引入（見「未來擴充」章節）

---

## Graceful Shutdown 設計

確保 Rust 程序在收到 SIGINT / SIGTERM 時，不中斷進行中的寫入與 AI 推論。

關閉順序（固定，不可顛倒）：
1. 停止接收新 HTTP 請求（Axum graceful shutdown）
2. 等待所有進行中的 handler 完成（含 AI 推論，10s timeout 維持有效）
3. Flush BulkInsertBuffer 剩餘資料
4. 關閉 PostgreSQL 連線池
5. 結束進程

```rust
// src/main.rs (Claude Code)

#[tokio::main]
async fn main() {
    // 初始化 tracing, DB pool, Redis, AI client...

    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for shutdown signal");
        tracing::info!("Shutdown signal received, starting graceful shutdown");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .expect("Server error");

    tracing::info!("All in-flight requests completed");

    bulk_insert_buffer.flush_and_close(&db_pool).await;
    tracing::info!("Bulk insert buffer flushed");

    db_pool.close().await;
    tracing::info!("Database connection pool closed");

    tracing::info!("Graceful shutdown complete");
}
```

Python AI Service 對應：
- FastAPI 同樣監聽 SIGTERM，完成進行中推論後才關閉
- 部署時 Python service 先於 Rust API 停止接收請求（見部署順序）

---

## 外部資料來源設計

### 主力：FinMind API

```rust
// src/data/fetch_rate_limiter.rs (Gemini 負責)
pub struct FinMindRateLimiter {
    config: RateLimitConfig,
    request_counter: AtomicU32,
    window_start: Instant,
}

pub struct RateLimitConfig {
    pub max_requests_per_minute: u32,
    pub max_requests_per_day: u32,
    pub upgrade_tier: ApiTier,
}

pub enum ApiTier {
    Free,
    Paid,
}
```

### 備用：yfinance

僅限排程補資料任務，禁止 handler 層直接呼叫，禁止放在即時資料路徑。

---

## Bulk Insert 設計

```rust
// src/data/db.rs (Gemini 負責)
// 攢批: 累積 500 筆或 1 秒，擇一觸發

pub struct BulkInsertBuffer {
    buffer: Vec<RawCandle>,
    last_flush_at: Instant,
    max_batch_size: usize,   // 500
    max_wait_ms: u64,        // 1000ms
}

impl BulkInsertBuffer {
    pub async fn flush(&mut self, db_pool: &PgPool) -> Result<(), DbError> {
        // INSERT ... ON CONFLICT (symbol, timestamp_ms, interval) DO NOTHING
    }

    // Graceful Shutdown 時呼叫
    pub async fn flush_and_close(&mut self, db_pool: &PgPool) -> Result<(), DbError> {
        self.flush(db_pool).await
    }
}
```

批次 COMMIT 後統一 DEL affected symbols 的 redis keys，不逐筆刷新。

---

## 動態 Symbol 清單設計

```
DB Table: symbols
  symbol          TEXT PRIMARY KEY
  name            TEXT
  exchange        TEXT
  data_source     TEXT
  earliest_ms     BIGINT
  latest_ms       BIGINT
  is_active       BOOLEAN
  updated_at_ms   BIGINT
```

每日 02:00 從 FinMind 同步。新增標的 is_active = true，下市標的 is_active = false（保留歷史）。

---

## 快取鍵設計

| Key 格式 | 寫入者 | 讀取者 | TTL | 失效觸發 |
|---------|--------|--------|-----|---------|
| stock:{symbol}:latest | Claude Code | Gemini, Codex | 5 分鐘 | 新 K 線寫入 |
| indicators:{symbol}:{interval} | Claude Code | Codex, Gemini | 10 分鐘 | 新 K 線寫入 |
| signal:{symbol}:* | Claude Code | Codex | 24 小時 | 新 K 線寫入 |
| symbols:active | Claude Code | Codex | 10 分鐘 | Symbol 同步完成 |
| backtest:{id} | Codex | Codex | 永久 | 手動刪除 |
| model:{version} | Codex | Gemini | 24 小時 | 模型更新 |

快取失效順序 (固定，不可顛倒):
1. INSERT / Bulk INSERT 寫入 DB
2. COMMIT 事務
3. DEL redis_key (批次統一執行)

---

## 指標 Factory 設計

```rust
// src/core/indicators/factory.rs (Claude Code)
pub struct IndicatorFactory {
    registry: HashMap<IndicatorId, Box<dyn IndicatorCalculator>>,
}

impl IndicatorFactory {
    pub fn resolve_execution_order(
        &self,
        requested_indicators: &[IndicatorConfig],
    ) -> Result<Vec<IndicatorId>, IndicatorError> {
        // 拓撲排序，循環依賴回傳 InvalidIndicatorConfig
    }

    pub fn compute_all(
        &self,
        candles: &[Candle],
        execution_order: &[IndicatorId],
    ) -> Result<HashMap<IndicatorId, IndicatorResult>, IndicatorError> {
        // 依序計算
    }
}
```

---

## Signal Channel 設計（預留 WebSocket 擴充）

```rust
// src/core/strategy/signal_aggregator.rs (Claude Code)
// 目前: oneshot channel 服務 REST handler
// 未來: 替換為 tokio::sync::broadcast::Sender<TradeSignalResponse>

pub struct SignalAggregator {
    sender: tokio::sync::oneshot::Sender<TradeSignalResponse>,
}
```

---

## 8 大協作陷阱與預防

### 1. 資料不一致
預防: Rust 獨家計算指標，Python 透過 API 消費
監控: CI 對比測試，差異 > 0.01% 需人工確認

### 2. API Breaking Change
預防: 版本化端點，breaking change 提前通知
流程: Gemini 提案 -> EJ 簽核 -> 三方並行遷移 -> 舊版 3 個月後棄用

### 3. 快取不一致
預防: 固定失效順序 (INSERT -> COMMIT -> DEL)，批次統一 DEL
監控: /health/integrity 每日比對

### 4. DB 事務不一致
預防: Bulk Insert 事務，ON CONFLICT DO NOTHING 冪等
恢復: 回滾至上一個良好狀態

### 5. 模型漂移
預防: 定期重訓，準確率 < 55% 自動告警
應急: 回滾至上一版本模型

### 6. 資源枯竭
預防: Rate Limiting，連線池
設定: AI 10s / DB 5s / Redis 1s timeout

### 7. 數值溢位
預防: Python is_finite() 與 i64 範圍檢查，Rust checked 運算
錯誤: COMPUTATION_OVERFLOW

### 8. 測試覆蓋不足
預防: Unit + Integration + Contract + E2E test
強制: CI 全部通過才允許 merge

---

## 成功指標

| 指標 | 目標 |
|------|------|
| Code Coverage | > 80% |
| Test Pass Rate | 100% |
| API P99 Latency | < 500ms |
| Cache Hit Rate | > 70% |
| Model Accuracy | > 65% |
| AI Service Uptime | > 99.5% |
| Max Drawdown (回測) | < 20% |
| Sharpe Ratio (回測) | > 1.0 |
| Data Latency | < 5 分鐘 (最新 K 線距當前時間) |
| AI Inference P99 | < 200ms |
| API Success Rate | > 99% |

---

## 未來擴充

### WebSocket 即時信號推送（2026 年度計畫）

需新增模組:
```
src/api/
  handlers/ws_signals.rs
  ws/
    connection_manager.rs
    broadcaster.rs
```

核心改動: signal_aggregator.rs 的 oneshot -> broadcast channel
前提: 整合測試穩定後 + EJ 簽核 WebSocket Spec
時程: 2026-06 ~ 2026-07

### Arrow 格式支援（待評估）

當 MsgPack 實測後仍為效能瓶頸，再由 EJ 決策引入 Apache Arrow。

---

版本歷史:
- 1.0 (2026-04-10): 初版
- 2.0 (2026-04-11): 合併 TECH_SPEC，更新角色分工
- 2.1 (2026-04-11): Bulk Insert、DataSource、Symbol 管理、FinMind 限流、WebSocket 預留
- 2.2 (2026-04-11): BridgeError 設計、序列化格式策略、Graceful Shutdown
- 2.3 (2026-04-11): Candle domain model 與 API response struct 分離，新增 IndicatorValue enum

批准: EJ (PM)
下次審查: 2026-04-25
