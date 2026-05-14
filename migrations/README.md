# TW Stock Analysis — DB Migrations

## 目錄結構

```
migrations/
├── 001_add_candle_audit_fields.sql   # candles 新增 created_at_ms、source
├── 001_rollback.sql
├── 002_add_symbols_finmind_earliest.sql  # symbols 新增 finmind_earliest_ms
├── 002_rollback.sql
├── 003_create_sync_log.sql           # 新增 sync_log table
├── 003_rollback.sql
├── 004_create_institutional_investors  # 新增 institutional_investors table
├── 004_rollback.sql
└── run_migrations.sh                 # 執行腳本
```

## 執行順序

**必須依序執行，不可跳過：**

```
001 → 002 → 003 → 004
```

## 使用方式

```bash
# 設定連線
export DATABASE_URL=postgres://user:pass@localhost:5432/ai_bridge

# 執行全部 migration
./run_migrations.sh

# 只執行指定編號
./run_migrations.sh 002

# 回滾指定編號（從最新往舊回滾）
./run_migrations.sh rollback 004
./run_migrations.sh rollback 003
./run_migrations.sh rollback 002
./run_migrations.sh rollback 001
```

## 各 Migration 說明

### 001 — candles table 新增欄位

| 欄位 | 型別 | 說明 |
|------|------|------|
| `created_at_ms` | BIGINT NOT NULL | 第一次寫入 DB 的時間戳（毫秒） |
| `source` | TEXT NOT NULL | 資料來源：`finmind` / `yfinance` |

現有資料：`created_at_ms` 預設為執行 Migration 的當下時間，`source` 預設為 `finmind`。

### 002 — symbols table 新增欄位

| 欄位 | 型別 | 說明 |
|------|------|------|
| `finmind_earliest_ms` | BIGINT NULL | FinMind 可提供的最早資料時間戳 |

由 `symbol_sync.rs` 每日同步時寫入，NULL 表示尚未查詢。

### 003 — sync_log table（新建）

記錄每次同步任務的完整歷史。

| 欄位 | 型別 | 說明 |
|------|------|------|
| `sync_id` | TEXT PK | 唯一識別碼 |
| `sync_type` | TEXT | `scheduled` / `manual` |
| `triggered_by` | TEXT | `system` / `ej` |
| `symbols` | TEXT[] | 本次同步股票清單 |
| `total_inserted` | INTEGER | 新增筆數 |
| `total_skipped` | INTEGER | 跳過筆數 |
| `total_failed` | INTEGER | 失敗筆數 |
| `started_at_ms` | BIGINT | 開始時間 |
| `completed_at_ms` | BIGINT NULL | 完成時間，NULL 表示未完成 |
| `status` | TEXT | `running` / `rate_limit_waiting` / `completed` / `failed` |

### 004 — institutional_investors table（新建）

記錄三大法人每日買賣超資料（寬表）。

| 欄位 | 型別 | 說明 |
|------|------|------|
| `symbol` | VARCHAR(20) PK | 股票代號 |
| `date` | DATE PK | 交易日期 |
| `foreign_investor_buy` | BIGINT | 外資買進股數 |
| `foreign_investor_sell` | BIGINT | 外資賣出股數 |
| `investment_trust_buy` | BIGINT | 投信買進股數 |
| `investment_trust_sell` | BIGINT | 投信賣出股數 |
| `dealer_self_buy` | BIGINT | 自營商買進股數 |
| `dealer_self_sell` | BIGINT | 自營商賣出股數 |
| `dealer_hedging_buy` | BIGINT | 自營商避險買進股數 |
| `dealer_hedging_sell` | BIGINT | 自營商避險賣出股數 |
| `foreign_dealer_self_buy` | BIGINT | 外資自營商買進股數 |
| `foreign_dealer_self_sell` | BIGINT | 外資自營商賣出股數 |
| `created_at_ms` | BIGINT | 第一次寫入時間戳（毫秒） |

## 注意事項

- 所有 Migration 均包在 `BEGIN / COMMIT` 事務中，失敗自動回滾
- `run_migrations.sh` 會自動記錄已執行的 Migration，重複執行安全
- 回滾時請從最新往最舊依序執行
- 生產環境執行前請先在 staging 驗證
