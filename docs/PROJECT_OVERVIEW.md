# AI Bridge - 專案概覽

版本: 2.0
更新日期: 2026-04-11
維護者: EJ (PM)

---

## 專案目標

構建一個準確、穩定、高效的股票分析與交易信號系統。

技術棧: Next.js + Rust/Axum + Python/FastAPI + PostgreSQL + Redis
時程: 2026-04-11 ~ 2026-12-31

---

## 團隊

| 角色 | 負責範圍 |
|------|---------|
| EJ (PM) | Spec 簽核、優先級、驗收、衝突調解 |
| Gemini CLI | PRD/API Schema 設計、src/data 數據層 |
| Claude Code | Rust 核心指標、API Gateway、中介軟體 |
| OpenAI Codex | Python AI 回測、前端 Dashboard |

---

## 核心設計原則

不可破壞的邊界:
- Rust 獨家計算所有技術指標，Python 透過 API 消費，不自行計算
- Python AI Service 不反向呼叫 Rust，資料流向單向
- 前端不持有業務邏輯，所有計算由後端提供
- 回測必須透過 POST /api/v1/indicators/compute 取得指標，確保與實盤一致

程式碼規範:
- 優先使用「自我解釋型」命名 (Self-Documenting Code)，禁止單字元或無意義縮寫
- 如需註解單行註解控制在 10 到 15 Words內，程式碼註解禁止使用 emoji
- 每個函數只做一件事，禁止 God function
- 優先考慮設計模式適用性 (Factory, Strategy, Observer 等)

---

## 系統架構 (精簡版)

```
Next.js 前端 (Codex)
    |
Rust API Gateway (Claude Code)
    |-- src/core:  技術指標計算 (Factory + DAG)
    |-- src/data:  外部 API 擷取、DB 寫入、快取失效 (Gemini)
    |-- src/api:   路由、中介軟體、錯誤處理
    |
    |-- PostgreSQL  歷史資料
    |-- Redis       指標快取
    |
Python AI Service (Codex)
    |-- /predict:  模型推論
    |-- 回測引擎:  呼叫 Rust API 取指標
```

詳細架構: ARCH_DESIGN.md
API 規範: API_CONTRACT.md

---

## 工期時程表

| 時間 | 里程碑 |
|------|-------|
| 2026-04-11 ~ 2026-04-17 | Spec 定稿由 EJ 簽核，Mock server 上線，各方建立 skeleton |
| 2026-04-18 ~ 2026-04-30 | src/data + src/core 核心功能，Python AI skeleton |
| 2026-05-01 ~ 2026-05-15 | API Gateway 完成，前端接 mock 完畢 |
| 2026-05-16 ~ 2026-05-31 | 整合測試，替換 mock，E2E 驗收 |
| 2026-06-01 ~ 2026-06-30 | 回測引擎，模型優化，效能測試 |
| 2026-07-01 ~ 2026-07-31 | 穩定期，Bug fix，生產環境就緒 |
| 2026-08 ~ 2026-12 | 持續監控、迭代、模型重訓排程 |

---

## 文件導航

| 文件 | 何時讀 | 誰讀 |
|------|--------|------|
| PROJECT_OVERVIEW.md (本文件) | 上手時、快速回顧 | 全員 |
| COLLAB_FRAMEWORK.md | 任務開始前 | 全員 |
| ARCH_DESIGN.md | 架構決策時 | 技術成員 |
| API_CONTRACT.md | 開發 API 時 | Gemini, Claude Code, Codex |
| DAILY_CHECKLIST.md | 提交、review、部署時 | 全員 |

---

## 如何開始

新功能開發流程:
1. 確認 PROJECT_OVERVIEW.md 了解全局
2. 閱讀 COLLAB_FRAMEWORK.md 確認自己的職責邊界
3. 等待 EJ 或自行提議 Spec，Gemini 撰寫，EJ 簽核
4. 依 DAILY_CHECKLIST.md 進行開發與提交

問題發生時:
- 技術問題: 查 ARCH_DESIGN.md
- API 格式問題: 查 API_CONTRACT.md
- 流程問題: 查 DAILY_CHECKLIST.md
- 決策衝突: 告知 EJ

---

版本歷史:
- 1.0 (2026-04-10): 初版
- 2.0 (2026-04-11): 簡化為導航用途，詳細內容移至各專門文件

批准: EJ (PM)
下次審查: 2026-04-25
