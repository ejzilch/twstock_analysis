-- Rollback: 004_rollback.sql
-- 回滾 004_create_institutional_investors.sql

BEGIN;

DROP INDEX IF EXISTS idx_institutional_investors_symbol_date;
DROP TABLE IF EXISTS institutional_investors;

COMMIT;