SET client_encoding = 'UTF8';
-- Migration: 002_add_symbols_finmind_earliest.sql
-- 目的: symbols table 新增 FinMind 最早可提供資料的時間戳
--       供缺口偵測邏輯（detect_gaps）判斷歷史補齊的起點
-- 執行順序: 001 之後，003 之前
-- 回滾腳本: 002_rollback.sql

BEGIN;

ALTER TABLE symbols
  ADD COLUMN IF NOT EXISTS finmind_earliest_ms BIGINT;

-- 允許 NULL：新股票同步前尚未取得此資訊
-- symbol_sync.rs 每日同步時一併更新此欄位

COMMENT ON COLUMN symbols.finmind_earliest_ms IS
  'FinMind API 可提供的最早歷史資料時間戳（毫秒）。
   由 symbol_sync.rs 每日同步時寫入。
   NULL 表示尚未查詢或 FinMind 無此標的資料。
   用於 manual_sync.rs detect_gaps() 計算缺口 A 的起點。';

COMMIT;
