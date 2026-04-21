SET client_encoding = 'UTF8';
-- Migration: 001_add_candle_audit_fields.sql
-- 目的: candles table 新增寫入時間與資料來源欄位
-- 執行順序: 必須在 002 之前執行
-- 回滾腳本: 001_rollback.sql

BEGIN;

-- 新增 created_at_ms：第一次寫入 DB 的時間戳（毫秒）
-- DEFAULT 設為目前時間，確保現有資料不會因 NOT NULL 失敗
ALTER TABLE candles
  ADD COLUMN IF NOT EXISTS created_at_ms BIGINT
    NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW()) * 1000;

-- 新增 source：資料來源識別
-- 現有資料預設標記為 finmind（原始排程資料來源）
ALTER TABLE candles
  ADD COLUMN IF NOT EXISTS source TEXT
    NOT NULL DEFAULT 'finmind';

-- 加上 CHECK 約束，確保 source 只接受合法值
ALTER TABLE candles
  ADD CONSTRAINT candles_source_check
    CHECK (source IN ('finmind', 'yfinance'));

-- 建立索引：供「查詢某次同步新增了哪些資料」使用
CREATE INDEX IF NOT EXISTS idx_candles_created_at_ms
  ON candles (created_at_ms);

CREATE INDEX IF NOT EXISTS idx_candles_source
  ON candles (source);

COMMIT;
