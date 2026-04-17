# AI Bridge — Frontend

## 技術棧
- Next.js 14 (App Router)
- TradingView Lightweight Charts
- TanStack Query v5
- Zustand
- TypeScript
- Tailwind CSS

## 快速開始

```bash
# 安裝依賴
npm install

# 設定環境變數
cp .env.local.example .env.local
# 編輯 .env.local，填入 NEXT_PUBLIC_API_BASE_URL 與 NEXT_PUBLIC_API_KEY

# 開發模式
npm run dev

# 型別生成（需要 OpenAPI spec）
npm run gen:types

# 建置
npm run build
```

## 目錄說明

```
src/
├── app/           # Next.js App Router 頁面
├── components/    # UI 元件（ui/ charts/ 為純 props，無 API 依賴）
├── hooks/         # React Query 資料層（所有 API 呼叫集中於此）
├── lib/           # 純工具函數（api-client, error-handler, utils）
├── store/         # Zustand UI 狀態（非伺服器資料）
├── types/         # TypeScript 型別定義
└── providers/     # React Provider 集中管理
```

## 層級規則

```
app/ → components/, hooks/, store/
components/ui/, charts/ → 純 props，無 hook/store 依賴
hooks/ → lib/api-client
lib/ → 純函數，無 React 依賴
store/ → UI 狀態，禁止存伺服器資料
```

## 環境變數

| 變數 | 說明 | 預設值 |
|------|------|--------|
| NEXT_PUBLIC_API_BASE_URL | Rust API Gateway 地址 | http://localhost:8080 |
| NEXT_PUBLIC_API_KEY | 開發用 API Key | dev-api-key |