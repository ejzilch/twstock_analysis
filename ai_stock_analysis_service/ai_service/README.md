# AI Bridge — Python AI Service

## 技術棧
- FastAPI + Uvicorn
- XGBoost（正式模型，2026-06 上線）
- 規則式邏輯（MVP 階段，預設啟用）
- MsgPack / JSON 自動序列化

## 快速開始

```bash
# 安裝依賴
pip install -r requirements.txt

# 啟動服務（預設 port 8001）
python -m ai_service.main

# 執行測試
pytest tests/ -v

# 型別檢查
mypy ai_service/

# 格式化
black ai_service/ tests/
```

## 環境變數

| 變數 | 說明 | 預設值 |
|------|------|--------|
| MODEL_TYPE | rules / xgboost | rules |
| PORT | 服務埠號 | 8001 |

## MVP → XGBoost 切換方式

```bash
# MVP 階段（預設）
MODEL_TYPE=rules python -m ai_service.main

# XGBoost 上線後（2026-06）
MODEL_TYPE=xgboost python -m ai_service.main
```

## 目錄說明

```
ai_service/
├── main.py           # FastAPI 入口，SIGTERM 處理
├── constants.py      # 所有業務數值常數
├── serialization.py  # JSON/MsgPack 自動切換
├── predict/
│   ├── handler.py    # /predict 端點邏輯
│   ├── features.py   # 特徵工程
│   └── validator.py  # is_finite() 數值安全檢查
├── backtest/
│   └── engine.py     # 回測引擎（指標由 Rust 提供）
└── models/
    └── model_registry.py  # 模型載入與版本管理
```

## 核心原則

- 回測指標**必須**由 Rust `POST /api/v1/indicators/compute` 提供，禁止自行計算
- 所有輸出數值回傳前須通過 `is_finite()` 與 i64 範圍檢查
- 非 2xx 回應一律使用 error envelope 格式（traceback 不對外暴露）
- SIGTERM 時等待進行中推論完成後才關閉