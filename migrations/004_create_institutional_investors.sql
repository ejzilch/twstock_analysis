SET client_encoding = 'UTF8';
-- Migration: 004_create_institutional_investors.sql
-- 目的: 新增三大法人買賣超資料表（寬表設計）
-- 執行順序: 003 之後
-- 回滾腳本: 004_rollback.sql

BEGIN;

CREATE TABLE IF NOT EXISTS institutional_investors (
  symbol                    VARCHAR(20)  NOT NULL,
  date                      DATE         NOT NULL,
  foreign_investor_buy      BIGINT       NOT NULL DEFAULT 0,
  foreign_investor_sell     BIGINT       NOT NULL DEFAULT 0,
  investment_trust_buy      BIGINT       NOT NULL DEFAULT 0,
  investment_trust_sell     BIGINT       NOT NULL DEFAULT 0,
  dealer_self_buy           BIGINT       NOT NULL DEFAULT 0,
  dealer_self_sell          BIGINT       NOT NULL DEFAULT 0,
  dealer_hedging_buy        BIGINT       NOT NULL DEFAULT 0,
  dealer_hedging_sell       BIGINT       NOT NULL DEFAULT 0,
  foreign_dealer_self_buy   BIGINT       NOT NULL DEFAULT 0,
  foreign_dealer_self_sell  BIGINT       NOT NULL DEFAULT 0,
  created_at_ms             BIGINT       NOT NULL,
  PRIMARY KEY (symbol, date)
);

CREATE INDEX IF NOT EXISTS idx_institutional_investors_symbol_date
  ON institutional_investors (symbol, date DESC);

COMMENT ON TABLE institutional_investors IS
  '三大法人每日買賣超資料（寬表）。
   資料來源：FinMind TaiwanStockInstitutionalInvestorsBuySell。
   由手動同步或排程同步寫入，ON CONFLICT DO NOTHING 保冪等。
   investor_type 共五種：Foreign_Investor、Investment_Trust、
   Dealer_self、Dealer_Hedging、Foreign_Dealer_Self。';

COMMIT;