#!/usr/bin/env bash
set -euo pipefail

DATABASES=(
  /vvtv/data/plans.sqlite
  /vvtv/data/queue.sqlite
  /vvtv/data/metrics.sqlite
  /vvtv/data/economy.sqlite
)

LOG_FILE="/vvtv/system/logs/sqlite_integrity.log"
mkdir -p /vvtv/system/logs >/dev/null 2>&1 || true
exec > >(tee -a "$LOG_FILE") 2>&1

echo "[$(date --iso-8601=seconds 2>/dev/null || date)] Iniciando verificação de integridade SQLite"

for db in "${DATABASES[@]}"; do
  if [[ ! -f $db ]]; then
    echo "[WARN] Banco ausente: $db"
    continue
  fi
  echo "[INFO] Verificando $db"
  result=$(sqlite3 "$db" "PRAGMA integrity_check;" 2>&1 || true)
  echo "[INFO] Resultado: $result"
  if [[ "$result" != "ok" ]]; then
    echo "[ERROR] Integridade falhou para $db"
  fi

done

echo "[$(date --iso-8601=seconds 2>/dev/null || date)] Verificação concluída"
