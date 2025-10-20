#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--target-hours <hours>] [--dry-run]

Reserva novos planos na fila de playout e dispara downloads/transcodes necessários para manter o buffer alvo.
USAGE
  exit 1
}

TARGET_HOURS=6
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target-hours)
      TARGET_HOURS="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "Opção desconhecida: $1" >&2
      usage
      ;;
  esac
done

log() {
  echo "[$(date --iso-8601=seconds 2>/dev/null || date)] $*"
}

require_sqlite() {
  if ! command -v sqlite3 >/dev/null 2>&1; then
    log "[ERROR] sqlite3 não encontrado"
    exit 1
  fi
}

require_sqlite

BUFFER=$(sqlite3 /vvtv/data/metrics.sqlite "SELECT buffer_duration_h FROM metrics ORDER BY ts DESC LIMIT 1;" 2>/dev/null || echo 0)
BUFFER=${BUFFER:-0}
log "Buffer atual: ${BUFFER}h"

if (( ${BUFFER%.*} >= TARGET_HOURS )); then
  log "Buffer atende ao alvo de ${TARGET_HOURS}h. Nada a fazer."
  exit 0
fi

NEEDED=$(echo "$TARGET_HOURS - $BUFFER" | bc -l)
log "Necessário preencher aproximadamente ${NEEDED}h"

SQL="WITH candidates AS (
  SELECT plan_id, title, duration_est_s
  FROM plans
  WHERE status = 'planned'
  ORDER BY curation_score DESC, created_at ASC
  LIMIT 50
)
SELECT plan_id, duration_est_s FROM candidates;"

mapfile -t rows < <(sqlite3 -csv /vvtv/data/plans.sqlite "$SQL")

if [[ ${#rows[@]} -eq 0 ]]; then
  log "[WARN] Nenhum plano disponível para seleção"
  exit 0
fi

TOTAL_ADDED=0
for row in "${rows[@]}"; do
  IFS=',' read -r plan_id duration <<<"$row"
  hours=$(echo "$duration / 3600" | bc -l)
  TOTAL_ADDED=$(echo "$TOTAL_ADDED + $hours" | bc -l)
  log "Selecionando plano $plan_id (${hours}h)"

  if ! $DRY_RUN; then
    sqlite3 /vvtv/data/queue.sqlite <<SQL
BEGIN;
INSERT INTO playout_queue (plan_id, asset_path, duration_s, status, curation_score)
VALUES ('$plan_id', '/vvtv/storage/ready/$plan_id.mp4', $duration, 'queued', 0);
UPDATE plans SET status = 'selected', updated_at = CURRENT_TIMESTAMP WHERE plan_id = '$plan_id';
COMMIT;
SQL
  fi

  if (( $(echo "$TOTAL_ADDED >= $NEEDED" | bc -l) )); then
    break
  fi

done

log "Total previsto adicionado ao buffer: ${TOTAL_ADDED}h"

if ! $DRY_RUN; then
  log "Acione o worker de download/transcode conforme pipeline do Epic C"
fi
