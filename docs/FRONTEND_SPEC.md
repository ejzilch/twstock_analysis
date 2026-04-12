# AI Bridge - 前端規格

版本: 1.0
更新日期: 2026-04-11
設計: EJ (PM)
實作: OpenAI Codex
狀態: 待 EJ 簽核

---

## 技術棧

| 項目 | 選擇 | 理由 |
|------|------|------|
| 框架 | Next.js (App Router) | 專案既定技術棧 |
| 圖表 | TradingView Lightweight Charts | 專案既定技術棧 |
| 伺服器狀態 | React Query (TanStack Query v5) | 內建輪詢、分頁、cache 控制，換 WebSocket 時只替換 fetcher |
| UI 狀態 | Zustand | 跨頁面共享 UI 狀態，輕量無 Provider，日後擴充成本低 |
| 型別 | TypeScript，interface 從 OpenAPI Spec 生成，禁止手寫 | |
| 樣式 | Tailwind CSS | |
| UI 狀態管理 | Zustand | 跨頁面共享 UI 狀態，輕量無 Provider，日後擴充成本低 |

---

## 目錄結構

```
frontend/
├── public/                         # 靜態資源（圖標、字體）
│
├── src/
│   ├── app/                        # Next.js App Router 路由層
│   │   ├── layout.tsx              # 全域佈局（側邊欄、主題、QueryClientProvider）
│   │   ├── page.tsx                # 根路徑，重定向至 /dashboard
│   │   ├── dashboard/
│   │   │   └── page.tsx            # Dashboard 頁（K 線 + 信號）
│   │   ├── stocks/
│   │   │   └── page.tsx            # 股票總覽頁
│   │   ├── backtest/
│   │   │   └── page.tsx            # 回測頁
│   │   └── settings/
│   │       └── page.tsx            # 設定頁
│   │
│   ├── components/                 # 元件庫
│   │   ├── ui/                     # 原子組件，無業務邏輯
│   │   │   ├── Button.tsx
│   │   │   ├── Card.tsx
│   │   │   ├── Input.tsx
│   │   │   ├── Toast.tsx           # error_code → UI 提示的統一入口
│   │   │   ├── Badge.tsx           # reliability badge
│   │   │   └── LoadingSpinner.tsx
│   │   │
│   │   ├── charts/                 # 圖表邏輯，只依賴 TradingView + props
│   │   │   ├── CandleChart.tsx     # 主 K 線圖，接受 candles + signals props
│   │   │   ├── IndicatorPane.tsx   # RSI / MACD 子圖
│   │   │   └── chartUtils.ts       # toTradingViewCandle()，格式轉換純函數
│   │   │
│   │   ├── dashboard/              # Dashboard 業務組件
│   │   │   ├── SymbolSelector.tsx  # 股票選擇器（從 /api/v1/symbols 載入）
│   │   │   ├── IntervalSelector.tsx
│   │   │   ├── SignalList.tsx      # 信號列表，含 reliability badge
│   │   │   └── PredictionPanel.tsx # AI 信心度面板
│   │   │
│   │   ├── stocks/                 # 股票總覽業務組件
│   │   │   ├── StockTable.tsx      # 股票清單表格，含搜尋篩選
│   │   │   └── StockRow.tsx        # 單行，含日線指標
│   │   │
│   │   ├── backtest/               # 回測業務組件
│   │   │   ├── StrategyForm.tsx    # 策略參數設定表單
│   │   │   ├── BacktestResult.tsx  # 結果卡片（win rate、sharpe 等）
│   │   │   └── BacktestChart.tsx   # 回測 K 線 + 信號疊加圖
│   │   │
│   │   └── settings/              # 設定頁業務組件
│   │       ├── ApiKeyForm.tsx
│   │       └── PreferenceForm.tsx
│   │
│   ├── hooks/                      # 自訂 Hooks，封裝 React Query 呼叫
│   │   ├── useSymbols.ts           # GET /api/v1/symbols
│   │   ├── useCandles.ts           # GET /api/v1/candles/{symbol}（含分頁）
│   │   ├── useSignals.ts           # GET /api/v1/signals/{symbol}
│   │   └── useBacktest.ts          # POST /api/v1/backtest（mutation）
│   │
│   ├── lib/                        # 核心工具，純函數或單例，無 React 依賴
│   │   ├── api-client.ts           # fetch 封裝，統一加 X-API-KEY header、處理 error_code
│   │   ├── error-handler.ts        # error_code → Toast / redirect 邏輯
│   │   └── utils.ts                # 日期格式化、數值格式化等通用工具
│   │
│   ├── store/                      # Zustand UI 狀態（非伺服器資料）
│   │   └── useAppStore.ts          # 統一單一 store
│   │
│   ├── types/                      # TypeScript 型別
│   │   ├── api.generated.ts        # 從 OpenAPI Spec 生成，禁止手動修改
│   │   └── app.ts                  # 業務層衍生型別（Pick<>/Omit<> 組合）
│   │
│   └── providers/                  # React Provider 集中管理
│       └── QueryClientProvider.tsx # React Query 全域設定
│
└── tailwind.config.ts
```

### 層級邊界規則

每一層只能往下依賴，禁止跨層或反向依賴：

```
app/ (路由層)
  └── 依賴 components/, hooks/, store/

components/ (元件層)
  ├── ui/        純 props，無任何 API 或 store 依賴
  ├── charts/    純 props，無任何 API 或 store 依賴
  └── 業務組件   可依賴 hooks/, store/, ui/, charts/

hooks/ (資料層)
  └── 依賴 lib/api-client.ts，封裝 React Query

lib/ (工具層)
  └── 純函數，無 React 依賴，可被任何層使用

store/ (UI 狀態層)
  └── 只存 UI 狀態，禁止存伺服器資料
```

### 各目錄職責說明

**`hooks/` vs `lib/` 邊界**

| 放 hooks/ | 放 lib/ |
|-----------|---------|
| 需要 React Query / useState 的邏輯 | 純函數，無 React 依賴 |
| `useCandles`、`useSignals` 等 | `toTradingViewCandle()`、`formatTimestamp()` |
| API 呼叫的 loading / error 狀態 | `api-client.ts` 的 fetch 封裝 |

**`store/useAppStore.ts` 只存以下狀態**

```typescript
interface AppState {
  selectedSymbol: string          // 目前選中的股票，預設 '2330'
  selectedInterval: string        // K 線時間粒度，預設 '1h'
  isEcoModeEnabled: boolean       // 收盤後節能模式，預設 true
  apiKey: string                  // 從 localStorage 初始化
  setSelectedSymbol: (symbol: string) => void
  setSelectedInterval: (interval: string) => void
  toggleEcoMode: () => void
  setApiKey: (key: string) => void
}
```

伺服器資料（candles、signals、symbols）一律交給 React Query 管理，禁止存入 store。

**`types/` 分工**

```typescript
// api.generated.ts — 機器生成，禁止手動修改
// 生成指令: npx openapi-typescript ./docs/openapi.yaml -o src/types/api.generated.ts
export type CandleResponse = components['schemas']['Candle']
export type SignalResponse = components['schemas']['TradeSignal']

// app.ts — 手寫業務衍生型別，從 generated 組合
import type { CandleResponse } from './api.generated'
export type CandleWithIndicators = CandleResponse & {
  indicators: Record<string, number | MacdValue>
}
```

---

## 頁面清單

| 路徑 | 頁面名稱 | 主要職責 |
|------|---------|---------|
| `/` | Dashboard | K 線圖表、即時信號、指標疊加 |
| `/stocks` | 股票總覽 | 股票清單、基本指標一覽 |
| `/backtest` | 回測 | 策略參數設定、回測結果圖表與指標 |
| `/settings` | 設定 | API Key 管理、顯示偏好 |

---

## 資料需求與 API 對應

### Dashboard (`/`)

| 功能 | API | 輪詢 | 前端 cache |
|------|-----|------|-----------|
| 股票選擇器 | GET /api/v1/symbols | 無（session 載入一次） | staleTime: 10 分鐘 |
| K 線圖表 | GET /api/v1/candles/{symbol} | 30 秒 | staleTime: 25 秒 |
| 技術指標疊加 | GET /api/v1/candles/{symbol}?indicators=ma20,rsi,macd | 同上，合併請求 | staleTime: 25 秒 |
| 即時信號 | GET /api/v1/signals/{symbol} | 30 秒 | staleTime: 25 秒 |
| AI 預測信心度 | 從 signals response 取得，不另打 API | — | — |

輪詢行為：
- 僅在頁面取得焦點（`refetchOnWindowFocus: true`）且 tab 可見時輪詢
- 頁面失去焦點超過 5 分鐘後暫停輪詢，恢復焦點時立即重新拉取
- 股市收盤時段（15:00 後）輪詢頻率降為 5 分鐘（由設定頁控制，預設開啟）

### 股票總覽 (`/stocks`)

| 功能 | API | 輪詢 | 前端 cache |
|------|-----|------|-----------|
| 股票清單 | GET /api/v1/symbols | 無（進頁載入） | staleTime: 10 分鐘 |
| 各股最新指標 | GET /api/v1/candles/{symbol}?interval=1d | 5 分鐘 | staleTime: 4 分鐘 |

分頁策略：
- 股票清單一次載入全部（預期數量在 1000 以內）
- 前端做搜尋與篩選，不打分頁 API

### 回測 (`/backtest`)

| 功能 | API | 說明 |
|------|-----|------|
| 策略執行 | POST /api/v1/backtest | 使用者按下「執行」才呼叫，非輪詢 |
| 歷史 K 線（結果疊加用） | GET /api/v1/candles/{symbol}（cursor 分頁） | 依回測時間範圍分批載入 |
| 歷史信號（結果疊加用） | GET /api/v1/signals/{symbol} | 同上時間範圍 |

注意事項：
- 回測為非同步操作，`POST /api/v1/backtest` 回應時間可能較長（依策略複雜度）
- 執行中顯示 loading state，完成後展示結果，不自動輪詢
- 回測結果 cache 於 React Query（`queryKey: ['backtest', backtest_id]`），使用者返回頁面不重新觸發

### 設定 (`/settings`)

| 功能 | 說明 |
|------|------|
| API Key 管理 | 儲存於 localStorage，每次請求從此取得放入 X-API-KEY header |
| 收盤後輪詢頻率 | 切換「收盤後節能模式」，影響 Dashboard 輪詢間隔 |
| 預設 interval | 使用者偏好的 K 線時間粒度，預設 1h |
| 預設 symbol | 進入 Dashboard 預設顯示的股票，預設 2330 |

---

## TradingView 整合規範

### 圖表初始化

```typescript
// 使用 TradingView Lightweight Charts，非 Widget（Widget 有 domain 限制）
import { createChart } from 'lightweight-charts';

// 圖表容器寬度隨父元素自動調整
// 高度固定 500px（Dashboard 主圖），200px（回測結果對比圖）
```

### K 線資料格式轉換

API 回傳的 candles 需轉換為 TradingView 格式：

```typescript
// API response -> TradingView CandlestickData
function toTradingViewCandle(candle: CandleResponse): CandlestickData {
  return {
    time: candle.timestamp_ms / 1000 as UTCTimestamp, // 秒級
    open: candle.open,
    high: candle.high,
    low: candle.low,
    close: candle.close,
  };
}
```

### 指標疊加規則

| 指標 | 疊加方式 | 顏色 |
|------|---------|------|
| MA5 | 主圖疊加線 | #2196F3 |
| MA20 | 主圖疊加線 | #FF9800 |
| MA50 | 主圖疊加線 | #9C27B0 |
| RSI | 獨立子圖（高度 100px） | #00BCD4 |
| MACD | 獨立子圖（高度 120px，含 histogram） | #F44336 / #4CAF50 |
| Bollinger Bands | 主圖疊加（上中下三線） | #607D8B |

### 信號標記疊加

```typescript
// BUY 信號: 綠色向上三角，顯示在 K 線下方
// SELL 信號: 紅色向下三角，顯示在 K 線上方
// reliability = low 時標記透明度降為 50%
// hover 顯示 tooltip: signal reason + confidence + source
const markers: SeriesMarker<UTCTimestamp>[] = signals.map(signal => ({
  time: signal.timestamp_ms / 1000 as UTCTimestamp,
  position: signal.signal_type === 'BUY' ? 'belowBar' : 'aboveBar',
  color: signal.signal_type === 'BUY' ? '#4CAF50' : '#F44336',
  shape: signal.signal_type === 'BUY' ? 'arrowUp' : 'arrowDown',
  text: `${signal.signal_type} ${(signal.confidence * 100).toFixed(0)}%`,
  size: signal.reliability === 'low' ? 1 : 2,
}));
```

---

## 輪詢策略

### 正常交易時段（09:00 ~ 15:00）

```typescript
useQuery({
  queryKey: ['candles', symbol, interval],
  queryFn: () => fetchCandles(symbol, interval),
  refetchInterval: 30_000,           // 30 秒
  staleTime: 25_000,                 // 25 秒內視為新鮮，不重複請求
  refetchOnWindowFocus: true,
  refetchIntervalInBackground: false, // 背景 tab 不輪詢
});
```

### 收盤時段（15:00 後，節能模式開啟）

```typescript
refetchInterval: 5 * 60_000, // 5 分鐘
```

### WebSocket 上線後的遷移方式

輪詢版本和 WebSocket 版本的差異只在 fetcher 層，元件不需改動：

```typescript
// 目前（輪詢）
const fetcher = () => fetch(`/api/v1/signals/${symbol}`).then(r => r.json());

// 未來（WebSocket）替換 fetcher，useQuery 的 select / onSuccess 邏輯不變
const fetcher = () => subscribeWebSocket(`/ws/signals/${symbol}`);
```

---

## error_code → UI 行為對應表

前端必須依 error_code 顯示對應提示，禁止統一顯示「發生錯誤」。

| error_code | UI 行為 |
|-----------|---------|
| UNAUTHORIZED | 跳轉至設定頁，顯示「請設定有效的 API Key」 |
| AI_SERVICE_TIMEOUT | Toast（橘色）：「AI 算力繁忙，目前顯示技術指標信號」，信號卡片顯示 reliability badge |
| AI_SERVICE_UNAVAILABLE | Toast（橘色）：「AI 服務暫停，請稍後」 |
| DATA_SOURCE_INTERRUPTED | Toast（紅色）：「數據源暫中斷，顯示快取數據」，圖表右上角顯示「資料可能延遲」標記 |
| DATA_SOURCE_RATE_LIMITED | Toast（橘色）：「資料來源切換備援中，可能短暫延遲」 |
| INDICATOR_COMPUTE_FAILED | Toast（紅色）：「指標計算異常，請重新整理」，提供重試按鈕 |
| COMPUTATION_OVERFLOW | Toast（紅色）：「計算數值異常，請聯繫支援」 |
| INVALID_INDICATOR_CONFIG | inline 錯誤（指標設定欄位旁）：顯示 API 回傳的具體訊息 |
| SYMBOL_NOT_FOUND | 股票選擇器顯示「找不到此股票」 |
| QUERY_RANGE_TOO_LARGE | inline 錯誤（日期選擇器旁）：「查詢範圍過大，請縮小時間區間或分批載入」 |
| CACHE_MISS_FALLBACK | 靜默，不顯示任何提示 |

### reliability badge 顯示規則

```typescript
// 信號卡片右上角顯示來源與可靠性
const badgeConfig = {
  high:    { label: 'AI 高信心', color: 'green' },
  medium:  { label: 'AI 中信心', color: 'yellow' },
  low:     { label: '技術指標', color: 'gray' },
  unknown: { label: '信號異常', color: 'red' },
};
```

---

## 狀態管理規範（React Query）

### Query Key 命名規則

```typescript
// 統一格式: [資源名稱, ...參數]
['symbols']                                    // GET /api/v1/symbols
['candles', symbol, interval, from_ms, to_ms] // GET /api/v1/candles/{symbol}
['signals', symbol, from_ms, to_ms]           // GET /api/v1/signals/{symbol}
['backtest', backtest_id]                      // POST /api/v1/backtest 結果
```

### Cache 時間規範

| 資料類型 | staleTime | cacheTime |
|---------|-----------|-----------|
| symbols 清單 | 10 分鐘 | 30 分鐘 |
| K 線 + 指標 | 25 秒 | 5 分鐘 |
| 交易信號 | 25 秒 | 5 分鐘 |
| 回測結果 | Infinity（不過期） | 1 小時 |

### 全域設定

```typescript
// providers/query-client.tsx
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 2,                      // 失敗最多重試 2 次
      refetchOnWindowFocus: true,
      refetchIntervalInBackground: false,
    },
  },
});
```

---

## TypeScript Interface 規範

- 所有 interface 從 OpenAPI Spec 生成，使用 `openapi-typescript` 工具
- 禁止手寫 interface
- 生成路徑: `src/types/api.generated.ts`，不直接修改此檔案
- 業務層使用 `Pick<>` / `Omit<>` 衍生型別，不重複定義

```bash
# 生成指令（納入 CI）
npx openapi-typescript ./docs/openapi.yaml -o src/types/api.generated.ts
```

---

## 禁止清單

| 違規 | 後果 |
|------|------|
| 統一顯示「發生錯誤」不依 error_code 區分 | PR 退回 |
| 寫死 symbol 清單，未從 /api/v1/symbols 載入 | PR 退回 |
| 手寫 TypeScript interface（應從 OpenAPI 生成） | Code review 拒絕 |
| 直接修改 api.generated.ts | Code review 拒絕 |
| 禁止 any 型別（無充分說明） | Code review 拒絕 |
| 前端自行計算任何技術指標 | PR 退回 |
| 直接呼叫 PostgreSQL | PR 退回 |
| 在元件內直接 fetch，不透過 hooks/ | Code review 要求重構 |
| ui/ 或 charts/ 元件內依賴 hooks 或 store | Code review 拒絕 |
| 伺服器資料存入 Zustand store | Code review 拒絕 |
| 輪詢在背景 tab 持續執行 | Code review 拒絕 |
| lib/ 內引入 React hook | Code review 拒絕 |

---

## PM 驗收項目

EJ 驗收時確認以下項目：

畫面完整性:
- [ ] 四個頁面均可正常進入
- [ ] Dashboard K 線圖正確顯示，指標疊加無異常
- [ ] 信號標記正確疊加於 K 線上，BUY/SELL 方向正確
- [ ] reliability badge 顯示正確

資料正確性:
- [ ] 股票選擇器從 /api/v1/symbols 動態載入，非寫死
- [ ] 切換股票後圖表與信號正確更新
- [ ] 回測頁執行後正確顯示績效指標

錯誤處理:
- [ ] 關閉 AI service，Dashboard 顯示 AI_SERVICE_UNAVAILABLE 提示，圖表不崩潰
- [ ] 輸入無效 API Key，正確跳轉設定頁
- [ ] 查詢過大時間範圍，顯示 QUERY_RANGE_TOO_LARGE 提示

輪詢行為:
- [ ] 30 秒自動更新（可用 Network tab 驗證）
- [ ] 切換至背景 tab 後輪詢停止

---

## 版本歷史

| 版本 | 日期 | 主要變更 |
|------|------|---------|
| 1.0 | 2026-04-11 | 初版，補齊前端 Spec，對齊 Spec-First 原則 |
| 1.1 | 2026-04-11 | 新增完整目錄結構、層級邊界規則、Zustand store 定義、types 分工 |

批准: EJ (PM)
下次審查: 2026-04-25
