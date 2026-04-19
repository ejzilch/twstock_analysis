# AI Bridge - 手動同步 Spec

**版本：** 1.1
**日期：** 2026-04-19
**起草：** Claude Code
**狀態：** 待 EJ 簽核

---

## 一、功能概述

在現有每日 02:00 自動排程的基礎上，新增手動觸發補資料功能。EJ 可在 Settings 頁搜尋並選擇特定股票，系統自動偵測 DB 缺口，只對缺口日期請求 FinMind API，節省 rate limit 配額。

---

## 二、已確認決策

| 項目 | 決策 |
|------|------|
| 觸發方式 | Settings 頁 UI 按鈕 |
| 補資料範圍 | 兩段式缺口偵測（歷史段 + 近期段） |
| K 線粒度 | 全部（1m / 5m / 15m / 1h / 4h / 1d） |
| 股票選擇 | 搜尋 + 多選標籤 |
| 重複資料 | 先查 DB 確認缺口再打 API，不浪費請求次數 |
| Rate limit | 自動排隊等待，1 小時後繼續，無需人工介入 |
| 回饋方式 | 背景執行，完成後顯示結果（總數 / 完成 / 失敗） |
| 中間缺漏掃描 | 日後再加，MVP 不實作 |
| DB 時間戳記 | candles 補 `created_at_ms` + `source`，新增 `sync_log` table |
| `src/data/` 重構 | 不需重構，在現有基礎上擴充 |

---

## 三、缺口偵測邏輯

### 3.1 兩段式缺口計算

```
對每一檔股票、每一個粒度執行：

SELECT MIN(timestamp_ms), MAX(timestamp_ms)
FROM candles
WHERE symbol = '2330' AND interval = '1d'

結果：
  db_oldest = MIN(timestamp_ms)  // DB 最舊資料
  db_newest = MAX(timestamp_ms)  // DB 最新資料

計算缺口：
  缺口 A（歷史段）= finmind_earliest ~ db_oldest - 1天
  缺口 B（近期段）= db_newest + 1天  ~ 今天

特殊情況：
  DB 完全無資料 → 全段補（finmind_earliest ~ 今天）
  缺口 A 已無缺口 → 跳過 A，只補 B
  缺口 B 已是今天 → 跳過 B，只補 A
  兩段都無缺口   → 該股票該粒度已完整，跳過
```

### 3.2 FinMind 最早可提供日期

由 Gemini 在 `symbol_sync.rs` 同步時一併寫入 DB。

`symbols` table 新增欄位：

| 欄位名稱 | 型別 | 說明 |
|---------|------|------|
| `finmind_earliest_ms` | BIGINT | FinMind 可提供的最早資料時間戳（毫秒） |

---

## 四、DB Schema 補充

### 4.1 candles table 新增欄位

現有 candles table 缺少「寫入時間」與「資料來源」紀錄，補充如下：

| 欄位名稱 | 型別 | 允許 NULL | 說明 |
|---------|------|----------|------|
| `created_at_ms` | BIGINT | 否 | 第一次寫入 DB 的時間戳（毫秒） |
| `source` | TEXT | 否 | 資料來源：`finmind` / `yfinance` |

**INSERT 語句更新：**

```sql
INSERT INTO candles (
  symbol, timestamp_ms, interval,
  open, high, low, close, volume,
  created_at_ms, source
)
VALUES (...)
ON CONFLICT (symbol, timestamp_ms, interval) DO NOTHING
-- ON CONFLICT DO NOTHING：重複資料直接跳過，不更新 created_at_ms
-- 確保 created_at_ms 永遠記錄「第一次寫入」的時間
```

**實際查詢範例：**

```sql
-- 查詢某次同步新增了哪些資料
SELECT COUNT(*) FROM candles
WHERE created_at_ms >= 1745020800000  -- 本次同步開始時間
AND source = 'finmind'

-- 確認某筆 K 線是什麼時候補進來的
SELECT created_at_ms, source FROM candles
WHERE symbol = '2330' AND timestamp_ms = 1704067200000
```

---

### 4.2 sync_log table（新增）

記錄每次同步（排程 + 手動）的完整歷史，供日後追蹤與「同步歷史」功能使用。

```sql
CREATE TABLE sync_log (
  sync_id          TEXT     PRIMARY KEY,
  sync_type        TEXT     NOT NULL,  -- 'scheduled' / 'manual'
  triggered_by     TEXT     NOT NULL,  -- 'system' / 'ej'
  symbols          TEXT[]   NOT NULL,  -- 本次同步的股票清單
  total_inserted   INTEGER  NOT NULL DEFAULT 0,
  total_skipped    INTEGER  NOT NULL DEFAULT 0,
  total_failed     INTEGER  NOT NULL DEFAULT 0,
  started_at_ms    BIGINT   NOT NULL,
  completed_at_ms  BIGINT,             -- NULL 表示未完成
  status           TEXT     NOT NULL   -- 'running' / 'completed' / 'failed'
);
```

現有 02:00 排程完成後，同樣寫一筆 `sync_type = 'scheduled'` 紀錄，統一管理所有同步歷史。

---

### 4.3 symbols table 補充

```sql
ALTER TABLE symbols
  ADD COLUMN finmind_earliest_ms BIGINT;
```

---

### 4.4 `src/data/` 擴充範圍評估

**結論：不需重構，全部在現有基礎上擴充。**

| 檔案 | 改動方式 | 工作量 |
|------|---------|--------|
| `models.rs` | RawCandle 新增 `created_at_ms`、`source` 欄位 | 低 |
| `db.rs` | INSERT 帶入新欄位；新增 sync_log 寫入邏輯 | 低 |
| `fetch_rate_limiter.rs` | 擴充 async 等待機制（記錄進度、等待後繼續） | 中 |
| `fetch.rs` | 新增 `fetch_range(symbol, from_ms, to_ms, interval)`，現有函數不動 | 中 |
| `symbol_sync.rs` | 同步時一併寫入 `finmind_earliest_ms` | 低 |
| `manual_sync.rs` | 全新新增（缺口偵測 + 分批請求 + 排隊邏輯） | 中 |

**共用 rate limit 計數器（重要）：**

```
排程（02:00）和手動同步共用同一個 FinMindRateLimiter 實例。

情境：
  02:00 排程已用 300 次
  EJ 白天手動觸發又用 300 次
  → 合計 600 次，正確觸發等待

這是預期行為，確保兩個任務合計不超過 FinMind 上限。
```

---

## 五、Rate Limit 排隊機制

### 5.1 FinMind 限制

```
600 次 / hour（免費方案）
```

### 5.2 計數與等待邏輯

```
每次成功請求 FinMind → 計數器 +1

計數器達 590 次（保留 10 次緩衝）：
  → 記錄當前進度（哪一檔、哪個粒度、補到哪一天）
  → 等待 3600 秒（1 小時）
  → 計數器歸零
  → 從記錄的進度繼續

等待期間：
  → 狀態顯示「Rate limit 等待中，XX 分鐘後繼續」
  → 不中斷、不需 EJ 手動重啟
```

### 5.3 預估時間計算

```
總請求次數 = 選擇股票數 × 6 粒度 × 缺口月數
預估小時數 = ceil(總請求次數 / 590)
```

---

## 六、API 規範

### 6.1 POST /api/v1/admin/sync

觸發手動同步。

**Request：**

```json
{
  "request_id": "req-20260419-ABC123",
  "symbols": ["2330", "2317"]
}
```

**Response 202（已接受，背景執行）：**

```json
{
  "sync_id": "sync-20260419-ABC123",
  "status": "running",
  "symbols": ["2330", "2317"],
  "estimated_requests": 936,
  "estimated_hours": 2,
  "started_at_ms": 1704067200000
}
```

**Response 409（已有同步執行中）：**

```json
{
  "error_code": "SYNC_ALREADY_RUNNING",
  "message": "A manual sync is already in progress. Check /api/v1/admin/sync/status.",
  "sync_id": "sync-20260419-XYZ",
  "fallback_available": false,
  "timestamp_ms": 1704067200000,
  "request_id": "req-20260419-ABC123"
}
```

---

### 6.2 GET /api/v1/admin/sync/status

查詢目前同步進度，前端每 10 秒輪詢一次。

**Response 200（執行中）：**

```json
{
  "sync_id": "sync-20260419-ABC123",
  "status": "running",
  "started_at_ms": 1704067200000,
  "rate_limit": {
    "used_this_hour": 487,
    "limit_per_hour": 600,
    "is_waiting": false,
    "resume_at_ms": null
  },
  "progress": [
    {
      "symbol": "2330",
      "name": "台積電",
      "status": "running",
      "gap_a": {
        "from_ms": 1262304000000,
        "to_ms": 1672444800000,
        "inserted": 3240,
        "skipped": 0,
        "failed": 0,
        "completed": false
      },
      "gap_b": {
        "from_ms": 1718496000000,
        "to_ms": 1745020800000,
        "inserted": 456,
        "skipped": 12,
        "failed": 0,
        "completed": true
      }
    },
    {
      "symbol": "2317",
      "name": "鴻海",
      "status": "pending",
      "gap_a": null,
      "gap_b": null
    }
  ],
  "summary": {
    "total_symbols": 2,
    "completed_symbols": 0,
    "total_inserted": 3696,
    "total_skipped": 12,
    "total_failed": 0
  }
}
```

**Response 200（rate limit 等待中）：**

```json
{
  "sync_id": "sync-20260419-ABC123",
  "status": "rate_limit_waiting",
  "rate_limit": {
    "used_this_hour": 590,
    "limit_per_hour": 600,
    "is_waiting": true,
    "resume_at_ms": 1704070800000
  },
  "progress": ["..."]
}
```

**Response 200（完成）：**

```json
{
  "sync_id": "sync-20260419-ABC123",
  "status": "completed",
  "started_at_ms": 1704067200000,
  "completed_at_ms": 1704081600000,
  "progress": ["..."],
  "summary": {
    "total_symbols": 2,
    "completed_symbols": 2,
    "total_inserted": 4200,
    "total_skipped": 12,
    "total_failed": 0
  }
}
```

**status 值定義：**

| status | 說明 |
|--------|------|
| `running` | 正常執行中 |
| `rate_limit_waiting` | 達到上限，等待中 |
| `completed` | 全部完成 |
| `failed` | 發生不可回復的錯誤 |

---

### 6.3 新增 error_code

| error_code | HTTP | 說明 | 前端顯示 |
|-----------|------|------|---------|
| `SYNC_ALREADY_RUNNING` | 409 | 已有同步執行中 | 「同步執行中，請稍候」 |
| `SYNC_NOT_FOUND` | 404 | sync_id 不存在 | 靜默，重新載入 |
| `FINMIND_UNAVAILABLE` | 503 | FinMind API 無法連線 | 「FinMind 暫時無法連線，請稍後重試」 |

---

## 七、前端 UI 規範

### 7.1 位置

Settings 頁（`/settings`）新增「資料同步」區塊，置於 `PreferenceForm` 下方。

元件路徑：`src/components/settings/ManualSyncPanel.tsx`

### 7.2 股票選擇器設計：搜尋 + 多選標籤

股票池可能超過 1,000 檔（上市 900+ / 上櫃 800+），勾選清單不可行。
採用「搜尋輸入 + 已選標籤」設計，支援代號與名稱模糊搜尋。

```
選擇股票

快捷：[ 前 10 大市值 ]  [ 全部清除 ]

搜尋：[ 輸入代號或名稱...         ]
       ↓ 輸入「台」後展開下拉
      ┌─────────────────────────┐
      │ 2330  台積電   TWSE     │  ← 點擊加入
      │ 2308  台達電   TWSE     │  ← 點擊加入
      │ 3045  台灣大   TWSE     │  ← 點擊加入
      └─────────────────────────┘
      最多顯示 10 筆，繼續輸入可縮小結果

已選擇（3 檔）：
  [ 2330 台積電 × ]  [ 2317 鴻海 × ]  [ 2454 聯發科 × ]

[ 開始同步 ]
```

**互動規則：**

| 操作 | 行為 |
|------|------|
| 輸入代號（如 `2330`） | 精確匹配，直接顯示該股票 |
| 輸入名稱（如 `台積`） | 模糊搜尋，列出所有符合股票 |
| 點擊搜尋結果 | 加入已選標籤，輸入框清空 |
| 點擊標籤 `×` | 從已選移除該股票 |
| 點擊「前 10 大市值」 | 一鍵加入前 10 大市值股票，已選的不重複加入 |
| 點擊「全部清除」 | 清空所有已選股票 |
| 股票已在已選中 | 搜尋結果顯示打勾，點擊無效果 |
| 未選任何股票 | 「開始同步」按鈕 disabled |

**資料來源：**
搜尋候選清單來自 `GET /api/v1/symbols`（已有的 hook `useSymbols()`），不另打 API。前端對 symbols 做本地過濾，無額外請求。

---

### 7.3 UI 狀態說明

**初始狀態：**

```
資料同步

快捷：[ 前 10 大市值 ]  [ 全部清除 ]

搜尋：[ 輸入代號或名稱... ]

已選擇（0 檔）：尚未選擇任何股票

[ 開始同步 ]  ← disabled
```

**執行中：**

```
同步執行中                                    [ 收合 ]

API 使用量   [=========>     ]  487 / 600 次
預估完成     約 2.5 小時

2330 台積電
  歷史段（缺口 A）  [=====>       ]  45%
  近期段（缺口 B）  完成 ✅  +456 筆

2317 鴻海    等待中...

⚠ Rate limit 後自動繼續，請勿關閉視窗
```

**Rate limit 等待中：**

```
⏳ 達到 FinMind 每小時上限（590 / 600 次）
   將於 47 分鐘後自動繼續，無需任何操作
```

**完成：**

```
✅ 同步完成

2330 台積電
  歷史段   新增 3,240 筆   跳過 0 筆
  近期段   新增 456 筆     跳過 12 筆

2317 鴻海
  歷史段   已是最早資料，跳過
  近期段   新增 2 筆       跳過 0 筆

合計：新增 3,698 筆   失敗 0 筆

[ 再次同步 ]
```

---

### 7.4 輪詢規則

```typescript
// 同步執行中：每 10 秒查一次進度
// status === 'completed' 或 'failed'：停止輪詢
// 使用者離開 Settings 頁：停止輪詢（refetchIntervalInBackground: false）
// 使用者返回 Settings 頁：若 sync_id 仍存在且未完成，恢復輪詢
```

---

### 7.5 新增 Hook

`src/hooks/useManualSync.ts`

```typescript
// useTriggerSync()
//   mutation，觸發 POST /api/v1/admin/sync
//   成功後將 sync_id 存入 Zustand store

// useSyncStatus(syncId: string | null)
//   query，輪詢 GET /api/v1/admin/sync/status
//   只在 syncId 存在且 status 為 running / rate_limit_waiting 時啟動輪詢
//   refetchInterval: 10_000
//   refetchIntervalInBackground: false
```

---

### 7.6 Zustand store 新增欄位

```typescript
// store/useAppStore.ts 新增：
interface AppState {
  // ...現有欄位...
  activeSyncId: string | null
  setActiveSyncId: (id: string | null) => void
}
```

---

### 7.7 元件拆分

| 元件 | 路徑 | 職責 |
|------|------|------|
| `ManualSyncPanel` | `components/settings/ManualSyncPanel.tsx` | 整體面板，組合子元件 |
| `SymbolSearchInput` | `components/settings/SymbolSearchInput.tsx` | 搜尋輸入框 + 下拉候選清單 |
| `SelectedSymbolTags` | `components/settings/SelectedSymbolTags.tsx` | 已選標籤列表，含移除按鈕 |
| `SyncProgress` | `components/settings/SyncProgress.tsx` | 執行中進度顯示 |
| `SyncResult` | `components/settings/SyncResult.tsx` | 完成結果顯示 |

---

## 八、各方實作範圍

### Gemini（`src/data/`）

| 檔案 | 工作內容 |
|------|---------|
| `src/data/manual_sync.rs` | `detect_gaps()` 查 DB MIN/MAX 計算缺口 A 和 B |
| `src/data/manual_sync.rs` | `fetch_and_insert_gap()` 對缺口分批請求 FinMind，INSERT ON CONFLICT DO NOTHING |
| `src/data/manual_sync.rs` | `RateLimitQueue` 計數器，達 590 次自動等待並記錄進度 |
| `src/data/models.rs` | RawCandle 新增 `created_at_ms`、`source` 欄位 |
| `src/data/db.rs` | INSERT 帶入新欄位；新增 sync_log 寫入邏輯 |
| `src/data/fetch.rs` | 新增 `fetch_range()` 函數，現有函數不動 |
| `src/data/fetch_rate_limiter.rs` | 擴充 async 等待機制 |
| `src/data/symbol_sync.rs` | 同步時一併寫入 `finmind_earliest_ms` |
| DB Migration | candles 新增欄位、新增 sync_log table、symbols 新增欄位 |

### Claude Code（`src/api/`）

| 檔案 | 工作內容 |
|------|---------|
| `src/api/handlers/admin_sync.rs` | `POST /api/v1/admin/sync` 接收請求、啟動背景 task、回傳 sync_id |
| `src/api/handlers/admin_sync.rs` | `GET /api/v1/admin/sync/status` 查詢進度、回傳 JSON |
| `src/constants.rs` | 新增 `SYNC_ALREADY_RUNNING`、`SYNC_NOT_FOUND`、`FINMIND_UNAVAILABLE` |
| Redis | 同步狀態存於 `admin_sync:{sync_id}`，TTL 24 小時 |

### Codex（前端）

| 檔案 | 工作內容 |
|------|---------|
| `src/components/settings/ManualSyncPanel.tsx` | 整體面板 |
| `src/components/settings/SymbolSearchInput.tsx` | 搜尋輸入框 + 下拉候選 |
| `src/components/settings/SelectedSymbolTags.tsx` | 已選標籤列表 |
| `src/components/settings/SyncProgress.tsx` | 執行中進度顯示 |
| `src/components/settings/SyncResult.tsx` | 完成結果顯示 |
| `src/hooks/useManualSync.ts` | `useTriggerSync` + `useSyncStatus` |
| `src/store/useAppStore.ts` | 新增 `activeSyncId` 欄位 |
| `src/app/settings/page.tsx` | 掛載 `ManualSyncPanel` |
| `src/lib/error-handler.ts` | 新增 3 個 error_code 對應 |
| `src/types/api.generated.ts` | 新增 `SyncStatus`、`SyncProgress` 型別 |

---

## 九、未來擴充（本次不實作）

| 功能 | 說明 |
|------|------|
| 中間缺漏掃描 | 逐日比對 DB 找出中間零散缺漏 |
| 同步歷史紀錄頁面 | 讀取 sync_log，顯示每次同步結果 |
| 付費 ApiTier 自動切換 | FinMind 付費後提升 rate limit，`fetch_rate_limiter.rs` 已預留介面 |
| 同步排程 UI 設定 | 讓 EJ 從 Settings 頁調整每日排程時間 |

---

## 十、簽核

| 角色 | 狀態 | 日期 |
|------|------|------|
| EJ (PM) | 待簽核 | |
| Gemini CLI | 待確認實作範圍 | |
| Claude Code | 待確認實作範圍 | |
| OpenAI Codex | 待確認實作範圍 | |

**下次審查：** 2026-04-25

---

## 版本歷史

| 版本 | 日期 | 變更內容 |
|------|------|---------|
| 1.0 | 2026-04-19 | 初版 |
| 1.1 | 2026-04-19 | 新增第四節 DB Schema 補充（candles 時間戳記、sync_log table）；確認 src/data/ 不需重構；股票選擇器改為搜尋 + 多選標籤設計；章節編號重整 |