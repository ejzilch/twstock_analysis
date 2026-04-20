-- Rollback: 003_rollback.sql
-- 回滾 003_create_sync_log.sql

BEGIN;

DROP INDEX IF EXISTS idx_sync_log_status;
DROP INDEX IF EXISTS idx_sync_log_sync_type;
DROP INDEX IF EXISTS idx_sync_log_started_at_ms;
DROP TABLE IF EXISTS sync_log;

COMMIT;
