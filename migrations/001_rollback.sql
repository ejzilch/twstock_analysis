-- Rollback: 001_rollback.sql
-- 回滾 001_add_candle_audit_fields.sql

BEGIN;

DROP INDEX IF EXISTS idx_candles_source;
DROP INDEX IF EXISTS idx_candles_created_at_ms;

ALTER TABLE candles
  DROP CONSTRAINT IF EXISTS candles_source_check;

ALTER TABLE candles
  DROP COLUMN IF EXISTS source;

ALTER TABLE candles
  DROP COLUMN IF EXISTS created_at_ms;

COMMIT;
