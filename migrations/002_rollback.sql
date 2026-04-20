-- Rollback: 002_rollback.sql
-- 回滾 002_add_symbols_finmind_earliest.sql

BEGIN;

ALTER TABLE symbols
  DROP COLUMN IF EXISTS finmind_earliest_ms;

COMMIT;
