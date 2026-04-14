# AI Bridge - 協作框架

版本: 3.2
更新日期: 2026-04-11
維護者: EJ (PM)

---

## 團隊結構

```
EJ (PM)
  職責: Spec 簽核、優先級決策、衝突調解、驗收
  不寫生產代碼

  Gemini CLI         Claude Code          OpenAI Codex
  PRD / API Schema   Rust 核心 + Gateway  Python AI + 前端
  src/data 數據層    src/core + src/api   ai_service/ + frontend/
```

---

## 角色職責

### Gemini CLI

負責範圍:
- 撰寫 PRD 與 OpenAPI 3.0 Spec
- 定義 TypeScript interface (供 Codex 使用)
- 定義 Rust struct (供 Claude Code 實作)
- src/data 序列化/反序列化 (Serde)
- RawCandle 接收與 DB 寫入邏輯，包含 Bulk Insert buffer 實作
- DataSource enum 定義與 fetch.rs 內部 normalization
- FinMind API 排程限流器 (fetch_rate_limiter.rs)，預留 ApiTier 付費升級介面
- yfinance 備用邏輯，僅限排程補資料，禁止放在即時路徑
- 動態 Symbol 清單同步 (symbol_sync.rs)，每日從 FinMind 更新
- 快取失效設計: INSERT -> COMMIT -> 批次統一 DEL redis keys

PM 驗收點:
- 文件是否完整覆蓋業務需求
- JSON 格式是否易於前端渲染
- 資料結構是否與 Spec 一致
- FinMind 限流器是否有明確的 ApiTier 切換點

禁止:
- 指標計算邏輯
- 前端代碼
- Python AI 邏輯
- yfinance 放在即時資料路徑

---

### Claude Code

負責範圍:
- src/core/mod.rs: BridgeError 統一錯誤列舉定義
- src/core: 所有技術指標計算 (MA, RSI, MACD, Bollinger Bands)
- 指標 Factory: DAG 拓撲排序
- POST /api/v1/indicators/compute 動態指標端點
- GET /api/v1/symbols 端點實作
- GET /api/v1/candles/{symbol} 分頁邏輯 (cursor, 2000 根上限)
- src/api: HTTP 路由、中介軟體 (Auth, Rate Limiting)
- 認證中介軟體: 驗證 X-API-KEY header
- 全域錯誤碼定義與實作
- Python AI Service 客戶端: BridgeError 轉換、MsgPack / JSON 自動選擇
- /health/integrity 端點實作，含 Observability 指標
- src/main.rs Graceful Shutdown 實作
- tracing crate 結構化日誌，Python traceback 寫入 log
- Rust 金融數值使用 checked_add / checked_mul

PM 驗收點:
- cargo build 與 cargo clippy 無錯誤
- BridgeError 各 variant 是否都有對應的 tracing log
- Graceful Shutdown 是否依正確順序執行
- API 文件是否清楚，方便 Codex 串接

禁止:
- 機器學習邏輯
- 前端代碼
- 在 handler 內直接寫 SQL
- 對外暴露 Python traceback 或內部 BridgeError 細節
- 禁止在程式碼中硬寫 API 錯誤碼字串，一律引用 src/constants.rs 的常數
- 禁止在 API 層使用 String 表示有限合法值的欄位，一律使用 src/api/models/enums.rs 的 enum

---

### OpenAI Codex

負責範圍 (Python AI):
- FastAPI /predict 端點，含 error envelope 格式回傳
- 特徵工程與模型訓練 (XGBoost)
- 回測引擎: 透過 POST /api/v1/indicators/compute 取得 Rust 指標，禁止自行計算
- 模型版本管理
- Python 輸出數值範圍限制 (is_finite() 檢查，i64 安全範圍)
- ai_service/serialization.py: 依 Content-Type 自動解析 JSON / MsgPack
- FastAPI SIGTERM 處理，完成推論後才關閉

負責範圍 (前端，依 FRONTEND_SPEC.md):
- 四個頁面實作: Dashboard / 股票總覽 / 回測 / 設定
- TradingView Lightweight Charts 整合，K 線、指標疊加、信號標記
- React Query 狀態管理，30 秒輪詢，背景 tab 暫停
- 股票選擇器從 GET /api/v1/symbols 動態載入，禁止寫死
- 依 error_code 顯示對應 UI 提示（詳見 FRONTEND_SPEC.md 對應表）
- reliability badge 顯示信號來源與可靠性
- TypeScript interface 從 OpenAPI Spec 生成，使用 openapi-typescript

PM 驗收點 (Python AI):
- 回測結果是否正確產出
- error envelope 格式是否符合約定
- MsgPack 解析是否正確

PM 驗收點 (前端):
- 四個頁面均可正常進入，無 console error
- Dashboard K 線圖正確顯示，BUY/SELL 信號標記方向正確
- 信號 reliability badge 顯示正確（high/medium/low/unknown）
- 關閉 AI service 時，前端顯示降級提示，圖表不崩潰
- 股票選擇器動態載入，非寫死
- 30 秒輪詢可在 Network tab 驗證，背景 tab 不觸發
- 所有 error_code 有對應 UI 提示，無統一「發生錯誤」

禁止:
- 在回測中自行計算技術指標
- 直接呼叫 PostgreSQL
- 自行實作 HTTP API gateway
- 寫死 symbol 清單
- 對前端暴露 Python traceback
- 前端自行計算任何技術指標或統計數值
- 在元件內直接 fetch，不透過 React Query
- 手寫 TypeScript interface
- 輪詢在背景 tab 持續執行

---

## Code Review 矩陣

| 審核者 | 被審核者 | 審核重點 |
|--------|---------|---------|
| Claude Code | Codex | Python 邏輯漏洞、error envelope 格式、MsgPack 實作 |
| Codex | Gemini | src/data JSON 是否易於前端渲染 |
| Gemini | Claude Code & Codex | 產出是否偏離原始 Spec |
| Codex | Claude Code | API 文件是否清楚，方便串接 |

---

## Spec-First 並行開發

所有功能開工前必須有 EJ 簽核的 Spec，目的是讓三方並行，無需互相等待。

流程:
1. Gemini 撰寫 OpenAPI Spec + TS interface + Rust struct
2. EJ 簽核
3. 三方同時開工:
   - Claude Code 實作 Rust API，contract test 驗證
   - Codex 對接 mock server 開發前端
   - Codex 實作 Python AI service
4. EJ 驗收: 替換 mock -> 真實 API，整合測試通過

強制規則:
- TS interface 從 OpenAPI Spec 生成，不手寫
- Contract test 在 CI 中強制執行，Spec 偏差即失敗
- Mock server 在 Spec 簽核後 24 小時內部署

---

## 技術規範

### 設計原則

- 每個函數只做一件事 (單一職責)，禁止 God function
- 優先考慮設計模式的適用性 (Factory, Strategy, Observer 等)，但不強求套用
- 變數與函數命名必須優先使用「自我解釋型」命名 (Self-Documenting Code)，禁止單一字元或無意義縮寫
- 如需註解單行註解控制在 10 到 15 Words內，程式碼內的註解禁止使用 emoji 符號，使用純文字描述

### Rust 具體規範

- 禁止 .unwrap() (除非有充分的安全論證並附上說明)
- 錯誤處理使用 anyhow 或 thiserror，並透過 ? 傳遞
- 所有 pub 函數必須有 doc comment (///)
- 金融數值計算使用 checked_add / checked_mul，溢位時回傳 COMPUTATION_OVERFLOW
- 指標 Factory 透過拓撲排序決定計算順序，循環依賴回傳 400 error
- candles 查詢超過 2000 根時回傳 400 QUERY_RANGE_TOO_LARGE，不自動截斷
- BridgeError 每個 variant 必須對應 tracing::error! 或 tracing::warn! 記錄+
- 有限合法值的欄位禁止使用 String，一律使用 src/models/enums.rs 或 src/api/models/enums.rs 的對應 enum
- 錯誤碼字串禁止硬寫，一律引用 src/constants.rs 的常數（待產出）

### Python 具體規範

- 所有函數與參數必須有 type hint
- 輸出數值傳入 Rust 前必須通過範圍檢查 (is_finite, i64 範圍)
- 禁止 hardcoded 數值，一律使用具名常數或設定檔
- 禁止在回測引擎中自行計算任何技術指標
- 非 2xx 回應必須使用 error envelope 格式，traceback 欄位為選填
- 需在 requirements.txt 中列入 msgpack
- 禁止 hardcoded 數值（原有規範已有，但需明確對齊 constants.rs 的概念）
- Python 端的業務數值定義於 ai_service/constants.py，與 Rust 端分開管理

### TypeScript 具體規範

- interface 從 OpenAPI Spec 生成，不手寫
- 禁止 any 型別 (除有充分說明)
- 根據 error_code 處理各種錯誤情境，禁止統一顯示「發生錯誤」
- symbol 清單從 GET /api/v1/symbols 動態載入，禁止前端寫死

---

## 技術風險規範

### 快取一致性 (G1)

寫入順序固定: Bulk INSERT -> COMMIT -> 統一 DEL affected symbols 的 redis keys
若 Redis DEL 失敗: 記錄 warning log，依賴 TTL 自然過期，不中斷主流程

### 時間戳精度 (G2)

所有 timestamp 欄位統一為毫秒級 UTC integer (13 位)

### 指標計算順序 (G3)

Factory 拓撲排序，Response 附帶 dag_execution_order 欄位供驗證

### 數值溢位防護 (E1)

Python: is_finite() 與 i64 範圍檢查
Rust: checked_add / checked_mul，溢位回傳 COMPUTATION_OVERFLOW

### 錯誤碼體系 (E2)

| error_code | 前端顯示 |
|-----------|---------|
| UNAUTHORIZED | 請確認 API 金鑰是否正確 |
| AI_SERVICE_TIMEOUT | AI 算力繁忙，使用技術指標信號 |
| AI_SERVICE_UNAVAILABLE | AI 服務暫停，請稍後 |
| DATA_SOURCE_INTERRUPTED | 數據源暫中斷，顯示快取數據 |
| DATA_SOURCE_RATE_LIMITED | 資料來源達到請求上限，系統切換備援中 |
| INDICATOR_COMPUTE_FAILED | 指標計算異常，請重新整理 |
| COMPUTATION_OVERFLOW | 計算數值異常，請聯繫支援 |
| CACHE_MISS_FALLBACK | (靜默，前端不顯示) |
| INVALID_INDICATOR_CONFIG | 顯示具體錯誤訊息 |
| QUERY_RANGE_TOO_LARGE | 查詢範圍過大，請縮小時間區間或使用分頁 |

### 實盤與回測一致性 (E3)

禁止: Codex 在回測中自行計算技術指標
強制: 透過 POST /api/v1/indicators/compute 取得 Rust 結果
CI 驗證: 差異 > 0.01% 需 EJ 確認

### 監控端點 (E4)

GET /health/integrity 包含 Observability 指標:
- data_latency_seconds: 最新 K 線時間與當前時間差值，警告閾值 300 秒
- ai_inference_p99_ms: AI 推論 P99 耗時，警告閾值 200ms
- api_success_rate_pct: 最近 1 小時成功率，警告閾值 99%
- bridge_errors_last_hour: BridgeError 發生次數，警告閾值 5 次

EJ 每日巡視，任一指標 warning 超過 1 小時開緊急會議

### 外部資料來源管理 (E5)

FinMind 主力，yfinance 備用，fetch.rs 內部 normalization。
排程限流，ApiTier 預留付費升級切換點。
yfinance 禁止即時路徑，禁止 handler 層直接呼叫。

### 動態 Symbol 管理 (E6)

每日 02:00 從 FinMind 同步，is_active 標記管理，前端動態載入。

### 錯誤傳播橋接 (E7)

Rust 與 Python 之間通訊錯誤統一以 BridgeError 分類。
Python traceback 完整寫入 tracing log，不對前端暴露。
Python 端必須使用 error envelope 格式回傳非 2xx 錯誤。
BridgeError 每個 variant 必須有對應的 tracing log，不允許靜默吞錯。

### 序列化格式管理 (E8)

Rust <-> Python 內部傳輸依資料量自動選擇格式:
- 一般請求 (candle_count <= 1000): JSON
- 大批量傳輸 (candle_count > 1000): MsgPack

對外 API 固定使用 JSON，不受影響。
Python 端需引入 msgpack 套件並列入 requirements.txt。
Arrow 格式保留為未來擴充，MVP 不引入。

### 優雅停機 (E9)

Rust main.rs 監聽 SIGINT / SIGTERM，關閉順序固定:
1. 停止接收新請求
2. 等待進行中 handler 完成
3. Flush BulkInsertBuffer
4. 關閉 DB 連線池
5. 結束進程

Python FastAPI 同樣監聽 SIGTERM，完成推論後才關閉。
強行終止視為嚴重事故，需 post-mortem。

---

## 禁止清單 (EJ 強制執行)

| 違規 | 後果 |
|------|------|
| 功能開工前無 EJ 簽核 Spec | 拒絕 merge |
| 無通知的 breaking changes | 拒絕 merge，強制回滾 |
| Python 回測自行計算指標 | PR 退回 |
| Cache 失效順序錯誤 | Code review 拒絕 |
| 前端未依 error_code 顯示提示 | PR 退回 |
| Rust 金融數值不用 checked 運算 | Code review 拒絕 |
| Python 輸出未做範圍限制 | PR 退回 |
| 未使用「自我解釋型」命名 (Self-Documenting Code) 和使用單字元或無意義縮寫 | Code review 拒絕 |
| 函數承擔多個職責 | Code review 要求拆分 |
| 註解單行註解未控制在 10 至 15 Words內或程式碼註解含 emoji | Code review 拒絕 |
| yfinance 放在即時資料路徑 | PR 退回 |
| 前端寫死 symbol 清單 | PR 退回 |
| candles 超過 2000 根未回傳 400 | Code review 拒絕 |
| BridgeError 未記錄 tracing log | Code review 拒絕 |
| 對前端暴露 Python traceback | PR 退回 |
| Python 非 2xx 未使用 error envelope | PR 退回 |
| Rust 強行終止未走 Graceful Shutdown | 視為嚴重事故 |
| 有限合法值欄位使用 String（如 interval、exchange、signal_type） | Code review 拒絕 |

---

## 工期時程表 (參考)

| 時間 | 里程碑 |
|------|-------|
| 2026-04-11 ~ 2026-04-17 | Spec 定稿並由 EJ 簽核，Mock server 上線，各方 skeleton 建立 |
| 2026-04-18 ~ 2026-04-30 | src/data + src/core 核心功能 (含 Bulk Insert, Symbol sync, FinMind 限流, BridgeError)，Python AI skeleton |
| 2026-05-01 ~ 2026-05-15 | API Gateway 完成 (含 /symbols, 分頁, 認證, Graceful Shutdown, Observability)，前端接 mock 完畢 |
| 2026-05-16 ~ 2026-05-31 | 整合測試，替換 mock，E2E 驗收 |
| 2026-06-01 ~ 2026-06-30 | 回測引擎，模型優化，效能測試，WebSocket Spec 撰寫 (Gemini) |
| 2026-07-01 ~ 2026-07-31 | WebSocket 實作 (Claude Code + Codex)，穩定期，生產環境就緒 |
| 2026-08 ~ 2026-12 | 持續監控、迭代、模型重訓 |

---

## 版本歷史

| 版本 | 日期 | 主要變更 |
|------|------|---------|
| 1.0 | 2026-04-10 | 初版 |
| 2.0 | 2026-04-10 | 納入 Gemini 技術建議 |
| 3.0 | 2026-04-11 | 角色依藍圖重組，EJ 風險建議，Spec-First，程式碼規範 |
| 3.1 | 2026-04-11 | FinMind 限流、Bulk Insert、動態 Symbol、WebSocket 計畫 |
| 3.2 | 2026-04-11 | BridgeError 規範、序列化格式管理、Graceful Shutdown、Observability |
| 3.3 | 2026-04-11 | Codex 前端職責補齊，新增前端 PM 驗收點與禁止清單，對齊 FRONTEND_SPEC.md |
| 3.4 | 2026-04-14 | 新增 enum 強型別規範，禁止硬寫有限合法值字串 |

批准: EJ (PM)
下次審查: 2026-04-25
