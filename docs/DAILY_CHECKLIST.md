# AI Bridge - 開發流程檢查清單

版本: 2.2
更新日期: 2026-04-11

---

## 功能開工前 (Feature Kickoff)

EJ 確認以下項目簽核後才能開工:

- [ ] Gemini 已完成 OpenAPI Spec 撰寫
- [ ] TypeScript interface 已從 Spec 生成
- [ ] Rust struct 已定義
- [ ] EJ 已簽核 Spec
- [ ] Mock server 已部署

---

## 提交前 (Before Commit)

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
npm run build
npm test
tsc --noEmit
```

程式碼自查:
- [ ] 每個函數只做一件事，無 God function
- [ ] 優先使用「自我解釋型」命名 (Self-Documenting Code)，禁止單字元或無意義縮寫
- [ ] 如需註解單行註解控制在 10 到 15 Words內，程式碼註解禁止使用 emoji
- [ ] Rust: 無 .unwrap()，pub function 有 doc comment
- [ ] Rust: 金融數值使用 checked_add / checked_mul
- [ ] Rust: candles 查詢超過 2000 根時回傳 400 QUERY_RANGE_TOO_LARGE
- [ ] Rust: BridgeError 每個 variant 已對應 tracing log，無靜默吞錯
- [ ] Rust: 對外 response 未暴露 Python traceback 或 BridgeError 內部細節
- [ ] Python: 所有函數有 type hint
- [ ] Python: 輸出數值已做範圍限制 (is_finite, i64 範圍)
- [ ] Python: 回測未自行計算指標 (必須呼叫 Rust API)
- [ ] Python: 非 2xx 回應使用 error envelope 格式
- [ ] Python: msgpack 已列入 requirements.txt
- [ ] Gemini: yfinance 未出現在即時資料路徑
- [ ] Gemini: Bulk Insert 批次 COMMIT 後才統一 DEL redis keys
- [ ] Frontend: symbol 清單從 GET /api/v1/symbols 動態載入，未寫死
- [ ] Frontend: 已處理 QUERY_RANGE_TOO_LARGE, DATA_SOURCE_RATE_LIMITED error_code
- [ ] 無 breaking changes，或已通知所有人並提供遷移計畫

Commit message 格式:
```
feat(core): implement RSI indicator with 14-period default

Added RSI calculation supporting configurable period length.
Includes edge case handling for insufficient data points.

Tests: test_rsi_standard_case, test_rsi_insufficient_data
Breaking changes: none
```

---

## Code Review 時 (As Reviewer)

架構:
- [ ] 邏輯放在正確的層 (依 COLLAB_FRAMEWORK.md 角色定義)
- [ ] 無跨界修改他人模組
- [ ] API 契約無破壞性變更

程式碼品質:
- [ ] 函數職責單一，無 God function
- [ ] 使用「自我解釋型」命名 (Self-Documenting Code)，無縮寫或單字元變數
- [ ] 無 emoji 出現在程式碼註解中
- [ ] Rust: 無 .unwrap()，錯誤正確傳遞
- [ ] Python: 數值輸出範圍檢查

錯誤橋接:
- [ ] BridgeError variant 是否涵蓋所有 Python 失敗情境
- [ ] 每個 BridgeError 是否有 tracing::error! 或 tracing::warn! 記錄
- [ ] Python traceback 是否只寫 log，未暴露給前端
- [ ] Python 非 2xx 是否使用 error envelope 格式

序列化:
- [ ] candle_count > 1000 時是否切換 MsgPack
- [ ] Python 端是否依 Content-Type header 正確解析

資料來源:
- [ ] fetch.rs: yfinance 未出現在即時路徑
- [ ] FinMindRateLimiter 有明確的 ApiTier 切換點

寫入與快取:
- [ ] Bulk Insert 使用 ON CONFLICT DO NOTHING 保證冪等性
- [ ] 批次 COMMIT 後才統一 DEL redis keys

API 正確性:
- [ ] /api/v1/candles 超過 2000 根回傳 400
- [ ] /api/v1/symbols 回傳動態清單
- [ ] 認證錯誤回傳 401，header 為 X-API-KEY

測試:
- [ ] 新增代碼有對應測試
- [ ] Coverage > 80%
- [ ] 邊界情境有測試 (空值、溢位、超時、rate limit 切換、BridgeError 各 variant)

---

## 部署前 (Pre-Deployment)

EJ 確認:
- [ ] 所有 tests 通過 (unit + integration + contract)
- [ ] /health/integrity 回傳 status: ok
- [ ] /health/integrity observability 所有指標 status: ok
- [ ] /health/integrity data_source.rate_limit_remaining_pct > 20%
- [ ] 無未通知的 breaking changes
- [ ] DB migration 已備份並測試回滾流程
- [ ] Spec 已更新 (若有 API 變更)
- [ ] symbols table migration 已執行
- [ ] Python requirements.txt 含 msgpack
- [ ] 生產環境 .env 已設定，所有欄位無空值
- [ ] API_KEY 已設定為隨機強密鑰（建議 openssl rand -hex 32）
- [ ] FINMIND_API_TOKEN 已確認有效，可正常呼叫 API
- [ ] .env 未提交進 git（確認 git status 無 .env）
- [ ] NEXT_PUBLIC_API_BASE_URL 指向正確的生產環境位址

部署順序 (固定，不可顛倒):
1. DB migrations (含 symbols table)
2. Python AI Service (等待 health check 通過)
3. Rust API (等待 5 分鐘 health check 通過)
4. Frontend

關閉順序 (Graceful Shutdown，固定，不可顛倒):
1. 停止 Rust API 接收新請求
2. 等待進行中 handler 完成
3. Flush BulkInsertBuffer
4. 關閉 DB 連線池
5. 停止 Python AI Service

部署後驗證:
- [ ] /api/v1/health 回傳 status: ok
- [ ] /health/integrity 回傳 status: ok，observability 全綠
- [ ] GET /api/v1/symbols 回傳非空清單
- [ ] 無新 error log 出現
- [ ] 監控關鍵指標 30 分鐘

---

## 緊急修復 (Hotfix)

1. 確認問題 (15 分鐘內):
   - 確認問題可重現
   - 確認影響範圍
   - 判斷是否需要立即回滾

2. 回滾條件 (任一即回滾):
   - /health/integrity 回傳 status: error
   - observability 任一指標 status: critical
   - API P99 > 2000ms 超過 10 分鐘
   - Error rate > 5%
   - bridge_errors_last_hour > 20

3. 修復流程:
   ```bash
   git checkout main
   git checkout -b hotfix/issue-description
   # 修復
   cargo test
   # 一人快速 review
   # 部署到 staging 驗證
   # 部署到 production
   ```

4. 事後必做:
   - 撰寫 Post-mortem (根本原因 + 解決方案 + 預防措施)
   - 更新相關文件
   - 補充測試覆蓋

---

## EJ 每日巡視

健康狀態:
- [ ] curl /health/integrity 確認 status: ok
- [ ] 確認 observability.data_latency_seconds < 300
- [ ] 確認 observability.ai_inference_p99_ms < 200
- [ ] 確認 observability.api_success_rate_pct > 99
- [ ] 確認 observability.bridge_errors_last_hour < 5

資料來源:
- [ ] 確認 data_source.rate_limit_remaining_pct > 30%
- [ ] 確認 data_source.primary 為 finmind（若顯示 yfinance 表示主力受限）

其他:
- [ ] 檢查 error log 是否有異常 error_code
- [ ] Model accuracy 是否在 65% 以上
- [ ] 確認 Symbol 同步 log 有無異常

異常處理:
- integrity degraded 超過 1 小時: 召集相關 AI 排查
- data_latency_seconds > 300: 確認 FinMind 資料抓取是否正常
- ai_inference_p99_ms > 200: Codex 確認模型是否異常
- bridge_errors_last_hour > 5: Claude Code 查 tracing log，確認 Python 是否崩潰或 OOM
- rate_limit_remaining_pct < 10%: 評估是否升級 FinMind 付費方案
- Model accuracy < 55%: Codex 回滾模型，排定重訓

---

## 問題排查速查

症狀: API 回傳 500
1. 查 Rust structured log (tracing 輸出)
2. 確認是否有 BridgeError log (python_error, python_traceback 欄位)
3. curl http://python-service:8001/health
4. psql -c "SELECT 1"
5. redis-cli ping

症狀: Python 崩潰或 OOM
1. 查 BridgeError::PythonConnectionLost log
2. 確認 Python container 記憶體用量
3. 重啟 Python service，確認 /health/integrity python_ai_service 恢復 ok

症狀: 指標數值與預期不符
1. 檢查 dag_execution_order
2. 確認 Rust 差異在 0.01% 以內
3. 執行指標單元測試

症狀: 前端看到舊數據
1. curl /health/integrity 確認 cache_db_consistency
2. 若 mismatch: 手動清除 Redis 相關 key

症狀: data_latency_seconds 異常偏高
1. 確認 FinMind 資料抓取排程是否正常執行
2. 確認 rate_limit_remaining_pct 是否耗盡
3. 若切換至 yfinance: 確認 yfinance 備用是否正常

症狀: 資料來源切換至 yfinance
1. 查 FinMind rate_limit_remaining_pct
2. 確認排程任務是否異常消耗配額
3. 評估升級付費方案

症狀: 前端股票選擇器為空
1. curl /api/v1/symbols 確認回傳
2. 確認 symbols table 有資料
3. 確認 Symbol 同步排程正常

症狀: 負責 AI 的查找方式
| 問題類型 | 負責者 |
|---------|--------|
| API 500 / Rust 邏輯 / BridgeError | Claude Code |
| 指標定義 / 資料結構 | Gemini CLI |
| FinMind 限流 / Symbol 同步 | Gemini CLI |
| Python 崩潰 / OOM / error envelope | OpenAI Codex |
| 模型準確率 / 回測 | OpenAI Codex |
| 前端顯示問題 | OpenAI Codex |
| 架構決策 / 衝突 | EJ (PM) |
| FinMind 付費升級決策 | EJ (PM) |

---

版本歷史:
- 1.0 (2026-04-10): 初版
- 2.0 (2026-04-11): 新增程式碼規範檢查項目
- 2.1 (2026-04-11): 資料來源、Bulk Insert、Symbol 同步、分頁、認證
- 2.2 (2026-04-11): BridgeError 檢查、Observability 指標巡視、Graceful Shutdown 關閉順序
- 2.3 (2026-04-11): 部署前新增環境變數檢查項目

批准: EJ (PM)
下次審查: 2026-04-25