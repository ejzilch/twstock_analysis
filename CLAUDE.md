# CLAUDE.md - AI Bridge 工作記憶

> Claude Code 每次 session 自動載入。完整文件在 docs/ 下的 *.md。

---

## 常用指令

```bash
# Rust
cargo run
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt

# Python
pytest tests/ --cov
mypy .
black .

# Frontend
npm run dev
npm run build && npm test && tsc --noEmit
```

---

## 我負責的模組 (Claude Code)

```
src/domain/              領域核心（純計算，零 I/O）
  backtest/              回測引擎 + 財務指標
  indicators/            技術指標（MA/RSI/MACD/Bollinger）
  signal/                信號聚合（AI signal / technical fallback）
  strategy/              交易策略（trend_follow/mean_reversion/breakout）

src/services/            業務流程協調（Service layer）
  admin_sync.rs          手動同步流程協調
  backtest.rs            回測流程協調
  candle.rs              K 線查詢與指標計算協調
  signal.rs              信號產生流程協調
  sync_state.rs          同步狀態 Redis 存取

src/api/                 API 層
  handlers/              薄 handler（parse → call service → return）
  middleware/            auth / rate_limit / error
  models/                request / response / enums

src/data/                資料層
  db.rs                  BulkInsertBuffer + sync_log CRUD
  fetch.rs               FinMind API 呼叫
  fetch_rate_limiter.rs  FinMind 限流器（async 等待機制）
  manual_sync.rs         缺口偵測 + 分批補資料
  symbol_sync.rs         Symbol 清單同步
  implementations.rs     PostgresDbWriter + RedisInvalidator
  traits.rs              DbWriter + CacheInvalidator traits

src/ai_client/           Python AI Service 客戶端
  client.rs              BridgeError 轉換
  serialization.rs       JSON / MsgPack 自動選擇

src/models/              領域模型
  candle.rs              Candle / IndicatorValue / MacdValue
  enums.rs               所有 enum（Interval/Exchange/SignalType 等）
  indicators.rs          ComputeIndicatorsRequest/Response
  symbol.rs              SymbolMeta

src/main.rs              Graceful Shutdown
src/app_state.rs         AppState 定義
src/constants.rs         所有常數定義
```

**不再負責的模組：** 無（已接手所有模組）

---

## 模組邊界速查

| 模組 | 負責人 | 狀態 |
|------|--------|------|
| src/domain/（所有核心計算） | Claude Code | ✅ 完成 |
| src/services/（業務協調） | Claude Code | ✅ 完成 |
| src/api/（API 層） | Claude Code | ✅ 完成 |
| src/data/（資料層） | Claude Code | ✅ 完成 |
| src/ai_client/ | Claude Code | ✅ 完成 |
| ai_service/（Python AI） | Codex | 🔄 進行中 |
| frontend/ | Codex | 🔄 進行中 |

**絕對禁止：** 在 handler 內直接寫 SQL / 機器學習邏輯 / 前端代碼

---

## 開工前確認

新功能開工前，必須確認：
- [ ] EJ 已簽核 Spec
- [ ] Mock server 已部署（若前端需要）

---

## 程式碼強制規範

### 通用
- 函數單一職責，禁止 God function
- 命名使用 Self-Documenting Code，禁止單字元或無意義縮寫
- 單行註解控制在 10~15 words，禁止 emoji

### Rust
- 禁止 `.unwrap()`（需安全論證才例外，並加說明）
- 錯誤處理用 `thiserror` / `anyhow`，透過 `?` 傳遞
- 所有 `pub` 函數必須有 `///` doc comment
- 金融數值用 `checked_add` / `checked_mul`，溢位回傳 `COMPUTATION_OVERFLOW`
- candles 超過 2000 根 → 回傳 `400 QUERY_RANGE_TOO_LARGE`，禁止截斷
- 有限合法值欄位使用 enum，不用 String
  - 跨層共用（Interval, Exchange, SignalType）-> src/models/enums.rs
  - API 層專用（HealthStatus, FetchSource）-> src/api/models/enums.rs
- 錯誤碼字串引用 src/constants.rs 常數，不硬寫
- 禁止魔法數字，具業務語義的數值一律引用 src/constants.rs
- 判斷原則：數字需要 comment 才能理解就是魔法數字

### BridgeError（Rust ↔ Python）
- 每個 variant 必須對應 `tracing::error!` 或 `tracing::warn!`，禁止靜默吞錯
- Python traceback 寫入 log，**禁止對外 response 暴露**
- 對前端只回傳降級行為（`source: technical_only`）

### 序列化格式
- `candle_count <= 1000` → JSON
- `candle_count > 1000` → MsgPack（Content-Type: application/msgpack）
- 對外 REST API 固定 JSON

---

## Graceful Shutdown 順序（固定，不可顛倒）

1. 停止接收新 HTTP 請求（Axum graceful shutdown）
2. 等待進行中 handler 完成（AI 10s timeout 維持有效）
3. Flush `BulkInsertBuffer`
4. 關閉 PostgreSQL 連線池
5. 結束進程

---

## 快取失效順序（固定，不可顛倒）

1. Bulk INSERT 寫入 DB
2. COMMIT 事務
3. 統一 UNLINK affected symbols 的 redis keys

---

## 錯誤碼速查

| error_code | HTTP |
|-----------|------|
| UNAUTHORIZED | 401 |
| AI_SERVICE_TIMEOUT | 504 |
| AI_SERVICE_UNAVAILABLE | 503 |
| DATA_SOURCE_INTERRUPTED | 503 |
| DATA_SOURCE_RATE_LIMITED | 503 |
| INDICATOR_COMPUTE_FAILED | 500 |
| CACHE_MISS_FALLBACK | 200 |
| COMPUTATION_OVERFLOW | 422 |
| INVALID_INDICATOR_CONFIG | 400 |
| SYMBOL_NOT_FOUND | 404 |
| QUERY_RANGE_TOO_LARGE | 400 |
| SYNC_ALREADY_RUNNING | 409 |
| SYNC_NOT_FOUND | 404 |

---

## 禁止清單（違反 = PR 退回或 Code review 拒絕）

- `.unwrap()` 無說明
- handler 內直接寫 SQL
- BridgeError variant 無 tracing log
- 對外 response 暴露 Python traceback
- candles 超過 2000 根未回傳 400
- Graceful Shutdown 未依順序執行
- 金融數值未用 checked 運算
- God function / 單字元命名 / emoji 註解
- 在程式碼中硬寫 API 錯誤碼字串
- API 層使用 String 表示有限合法值

---

## 提交前自查（精簡版）

- [ ] `cargo clippy -- -D warnings` 無錯誤
- [ ] 所有 `pub fn` 有 `///`
- [ ] BridgeError 各 variant 有 tracing log
- [ ] 序列化格式依 candle_count 選擇正確
- [ ] 無 breaking changes，或已通知並附遷移計畫
- [ ] API 錯誤碼字串引用 src/constants.rs
- [ ] 有限合法值欄位已使用 enum，無裸字串
- [ ] 新增 enum 已放在正確的層（models/ 或 api/models/）
- [ ] 無魔法數字，業務數值已引用 constants.rs

---

## 完整文件位置

| 需要查什麼 | 查哪裡 |
|-----------|--------|
| 完整架構、BridgeError 設計、Graceful Shutdown | docs/ARCH_DESIGN.md |
| 所有 API 端點格式、error envelope | docs/API_CONTRACT.md |
| 角色職責邊界、技術風險規範 | docs/COLLAB_FRAMEWORK.md |
| 部署順序、EJ 巡視、問題排查 | docs/DAILY_CHECKLIST.md |
| 前端規格 | docs/FRONTEND_SPEC.md |
| 手動同步規格 | docs/MANUAL_SYNC_SPEC.md |
| 訓練標記定義 | docs/LABEL_DEFINITION_SPEC.md |