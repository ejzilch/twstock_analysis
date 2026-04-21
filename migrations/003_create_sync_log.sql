SET client_encoding = 'UTF8';
-- Migration: 003_create_sync_log.sql
-- 目的: 新增 sync_log table，記錄每次同步（排程 + 手動）的完整歷史
--       供狀態查詢、結果追蹤、日後「同步歷史」功能使用
-- 執行順序: 002 之後
-- 回滾腳本: 003_rollback.sql

BEGIN;

CREATE TABLE IF NOT EXISTS sync_log (
  -- 唯一識別碼，格式: sync-{YYYYMMDD}-{RANDOM6}
  -- 範例: sync-20260419-ABC123
  sync_id          TEXT        NOT NULL PRIMARY KEY,

  -- 同步類型：排程 or 手動
  sync_type        TEXT        NOT NULL,
  CONSTRAINT sync_log_sync_type_check
    CHECK (sync_type IN ('scheduled', 'manual')),

  -- 觸發者
  triggered_by     TEXT        NOT NULL,
  CONSTRAINT sync_log_triggered_by_check
    CHECK (triggered_by IN ('system', 'ej')),

  -- 本次同步的股票代號清單
  symbols          TEXT[]      NOT NULL,

  -- 統計數字（執行過程中逐步更新）
  total_inserted   INTEGER     NOT NULL DEFAULT 0,
  total_skipped    INTEGER     NOT NULL DEFAULT 0,
  total_failed     INTEGER     NOT NULL DEFAULT 0,

  -- 時間戳記
  started_at_ms    BIGINT      NOT NULL,
  completed_at_ms  BIGINT,     -- NULL 表示尚未完成

  -- 執行狀態
  status           TEXT        NOT NULL DEFAULT 'running',
  CONSTRAINT sync_log_status_check
    CHECK (status IN ('running', 'rate_limit_waiting', 'completed', 'failed'))
);

-- 查詢最近同步紀錄用
CREATE INDEX IF NOT EXISTS idx_sync_log_started_at_ms
  ON sync_log (started_at_ms DESC);

-- 查詢特定類型的同步紀錄用
CREATE INDEX IF NOT EXISTS idx_sync_log_sync_type
  ON sync_log (sync_type);

-- 查詢進行中的同步用（GET /api/v1/admin/sync/status）
CREATE INDEX IF NOT EXISTS idx_sync_log_status
  ON sync_log (status)
  WHERE status IN ('running', 'rate_limit_waiting');

COMMENT ON TABLE sync_log IS
  '每次同步任務（排程 + 手動）的完整歷史紀錄。
   手動同步：由 POST /api/v1/admin/sync 觸發，sync_type = manual。
   排程同步：由每日 02:00 cron 觸發，sync_type = scheduled。
   執行中狀態同時存於 Redis（key: admin_sync:{sync_id}，TTL 24h），
   Redis 作為即時查詢快取，sync_log 作為永久歷史紀錄。';

COMMIT;
