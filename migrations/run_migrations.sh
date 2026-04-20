#!/usr/bin/env bash
# run_migrations.sh
# 依序執行所有 migration，失敗時停止並提示回滾指令
#
# 使用方式:
#   ./run_migrations.sh                      # 執行全部
#   ./run_migrations.sh 001                  # 只執行指定編號
#   ./run_migrations.sh rollback 003         # 回滾指定編號
#
# 環境變數（請先設定）:
#   DATABASE_URL=postgres://user:pass@host:5432/ai_bridge

# ── 精確抓取 DATABASE_URL ──────────────────────────────────────────────────────

ENV_FILE=".env"

if [ -f "$ENV_FILE" ]; then
  # 使用 sed 尋找以 DATABASE_URL 開頭的行，並提取等號後面的內容
  # 這樣可以避免匯入其他變數，也比較安全
  DB_URL_FROM_ENV=$(grep '^DATABASE_URL=' "$ENV_FILE" | cut -d '=' -f2- | tr -d '"' | tr -d "'")

  if [ -n "$DB_URL_FROM_ENV" ]; then
    export DATABASE_URL="$DB_URL_FROM_ENV"
    echo "📂 已從 .env 載入 DATABASE_URL"
  fi
fi

# 檢查是否最終有取得變數
if [ -z "${DATABASE_URL:-}" ]; then
  echo "❌ 錯誤：找不到 DATABASE_URL。請在 .env 中設定或手動 export。"
  exit 1
fi

set -euo pipefail

MIGRATIONS_DIR="$(cd "$(dirname "$0")" && pwd)"
DATABASE_URL="${DATABASE_URL:-}"

# ── 前置檢查 ──────────────────────────────────────────────────────────────────

if [ -z "$DATABASE_URL" ]; then
  echo "❌ 錯誤：請先設定 DATABASE_URL 環境變數"
  echo "   範例: export DATABASE_URL=postgres://user:pass@localhost:5432/ai_bridge"
  exit 1
fi

command -v psql >/dev/null 2>&1 || {
  echo "❌ 錯誤：找不到 psql，請先安裝 PostgreSQL client"
  exit 1
}

# ── Migration 追蹤 table ──────────────────────────────────────────────────────

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
CREATE TABLE IF NOT EXISTS schema_migrations (
  migration_id   TEXT        PRIMARY KEY,
  executed_at_ms BIGINT      NOT NULL,
  checksum       TEXT        NOT NULL
);
SQL

# ── 函數：執行單一 migration ──────────────────────────────────────────────────

run_migration() {
  local file="$1"
  local migration_id
  migration_id=$(basename "$file" .sql)
  local checksum
  checksum=$(md5sum "$file" | awk '{print $1}')

  # 已執行過則跳過
  local already_run
  already_run=$(psql "$DATABASE_URL" -tAq \
    -c "SELECT COUNT(*) FROM schema_migrations WHERE migration_id = '$migration_id'")

  if [ "$already_run" -gt 0 ]; then
    echo "⏭  跳過 $migration_id（已執行）"
    return
  fi

  echo "▶  執行 $migration_id..."
  psql "$DATABASE_URL" -f "$file"

  # 記錄執行紀錄
  psql "$DATABASE_URL" -q <<SQL
INSERT INTO schema_migrations (migration_id, executed_at_ms, checksum)
VALUES ('$migration_id', EXTRACT(EPOCH FROM NOW()) * 1000, '$checksum');
SQL

  echo "✅ $migration_id 完成"
}

# ── 函數：回滾單一 migration ──────────────────────────────────────────────────

run_rollback() {
  local target_id="$1"
  local rollback_file="$MIGRATIONS_DIR/${target_id}_rollback.sql"

  if [ ! -f "$rollback_file" ]; then
    echo "❌ 找不到回滾腳本：$rollback_file"
    exit 1
  fi

  echo "⏪ 回滾 $target_id..."
  psql "$DATABASE_URL" -f "$rollback_file"

  psql "$DATABASE_URL" -q \
    -c "DELETE FROM schema_migrations WHERE migration_id LIKE '${target_id}%';"

  echo "✅ $target_id 回滾完成"
}

# ── 主程式 ────────────────────────────────────────────────────────────────────

MODE="${1:-all}"
TARGET="${2:-}"

if [ "$MODE" = "rollback" ]; then
  if [ -z "$TARGET" ]; then
    echo "❌ 請指定要回滾的 migration 編號，例如: ./run_migrations.sh rollback 003"
    exit 1
  fi
  run_rollback "$TARGET"
  exit 0
fi

# 執行模式：all 或指定編號
echo "🚀 開始執行 DB Migration"
echo "   DATABASE_URL: ${DATABASE_URL//:*@/:***@}"  # 隱藏密碼

for file in "$MIGRATIONS_DIR"/[0-9][0-9][0-9]_*.sql; do
  # 跳過 rollback 腳本
  [[ "$file" == *_rollback.sql ]] && continue

  migration_id=$(basename "$file" .sql | cut -d_ -f1)

  # 若指定了特定編號，只執行該編號
  if [ -n "$TARGET" ] && [ "$migration_id" != "$TARGET" ]; then
    continue
  fi

  run_migration "$file"
done

echo ""
echo "🎉 Migration 完成"
psql "$DATABASE_URL" -c \
  "SELECT migration_id, to_timestamp(executed_at_ms/1000)::TEXT AS executed_at
   FROM schema_migrations ORDER BY executed_at_ms;"
