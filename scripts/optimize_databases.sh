#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="${1:-/vvtv/data}"

if [[ ! -d "$DATA_DIR" ]]; then
  echo "error: data directory not found: $DATA_DIR" >&2
  exit 1
fi

shopt -s nullglob
found=false
for db in "$DATA_DIR"/*.sqlite; do
  found=true
  echo "🔧 optimizing $db"
  sqlite3 "$db" "PRAGMA journal_mode=WAL;" >/dev/null
  sqlite3 "$db" "PRAGMA synchronous=NORMAL; PRAGMA cache_size=-64000; PRAGMA temp_store=MEMORY; PRAGMA mmap_size=30000000000; PRAGMA busy_timeout=5000;" >/dev/null
  sqlite3 "$db" "PRAGMA wal_checkpoint(TRUNCATE); PRAGMA optimize; VACUUM;"
  sqlite3 "$db" "ANALYZE;"
  journal=$(sqlite3 "$db" "PRAGMA journal_mode;")
  pages=$(sqlite3 "$db" "PRAGMA page_count;")
  echo "✅ optimized $db (journal_mode=${journal}; page_count=${pages})"
done
shopt -u nullglob

if [[ "$found" == false ]]; then
  echo "no SQLite databases found in $DATA_DIR"
fi
