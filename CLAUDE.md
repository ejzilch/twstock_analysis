# CLAUDE.md - AI Bridge 工作記憶

> Claude Code 每次 session 自動載入。完整文件在 docs/ 下的 *.md。

---

## 常用指令

```bash
# Rust
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt

# Python
pytest tests/ --cov
mypy .
black .

# Frontend
npm run build && npm test && tsc --noEmit
```

---

## 我負責的模組 (Claude Code)

```
src/core/mod.rs          BridgeError 定義
src/core/indicators/     MA / RSI / MACD / Bollinger，Factory + DAG 拓撲排序
src/core/strategy/       signal_aggregator.rs (oneshot channel，預留 broadcast)
src/api/handlers/        candles / indicators / signals / health / symbols
src/api/middleware/      auth (X-API-KEY) / rate_limit / error
src/ai_client/           client.rs + serialization.rs (JSON / MsgPack 自動選擇)
src/main.rs              Graceful Shutdown
```

**不碰的模組：** `src/data/`（Gemini）、`ai_service/`（Codex）、`frontend/`（Codex）

---

## 模組邊界速查

| 模組 | 負責人 |
|------|--------|
| src/data/，RawCandle，Bulk Insert，FinMind 限流，Symbol 同步 | Gemini |
| src/core/，src/api/，ai_client/，main.rs | Claude Code (我) |
| ai_service/，frontend/ | Codex |

**絕對禁止：** 在 handler 內直接寫 SQL / 機器學習邏輯 / 前端代碼

---

## 開工前確認

新功能開工前，必須確認：
- [ ] EJ 已簽核 Spec
- [ ] Gemini 已定義對應的 Rust struct
- [ ] Mock server 已部署

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
3. 統一 DEL affected symbols 的 redis keys

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

---

## 提交前自查（精簡版）

- [ ] `cargo clippy -- -D warnings` 無錯誤
- [ ] 所有 `pub fn` 有 `///`
- [ ] BridgeError 各 variant 有 tracing log
- [ ] 序列化格式依 candle_count 選擇正確
- [ ] 無 breaking changes，或已通知並附遷移計畫

---

## 完整文件位置

| 需要查什麼 | 查哪裡 |
|-----------|--------|
| 完整架構、BridgeError 設計、Graceful Shutdown | docs/ARCH_DESIGN.md |
| 所有 API 端點格式、error envelope | docs/API_CONTRACT.md |
| 角色職責邊界、技術風險規範 | docs/COLLAB_FRAMEWORK.md |
| 部署順序、EJ 巡視、問題排查 | docs/DAILY_CHECKLIST.md |
