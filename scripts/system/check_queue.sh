#!/usr/bin/env bash
# VVTV Queue Inspector — provides queue health and buffer summary

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

VVTV_DATA_DIR=${VVTV_DATA_DIR:-/vvtv/data}
QUEUE_DB=${QUEUE_DB:-"$VVTV_DATA_DIR/queue.sqlite"}
RECENT=10
OUTPUT_JSON=0

usage() {
  cat <<'USAGE'
Usage: $(basename "$0") [--recent N] [--json]

Options:
  --recent N   Mostrar os N itens mais recentes na fila (padrão: 10)
  --json       Emitir resumo final em JSON (stdout)
  --help       Exibir esta mensagem
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --recent)
      RECENT="$2"
      shift 2
      ;;
    --json)
      OUTPUT_JSON=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Opção desconhecida: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ ! -f "$QUEUE_DB" ]]; then
  echo "Queue database não encontrado em $QUEUE_DB" >&2
  incident_log_append "check_queue.sh" "error" "queue.sqlite ausente" "$QUEUE_DB"
  exit 2
fi

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 não encontrado no PATH" >&2
  incident_log_append "check_queue.sh" "error" "sqlite3 ausente" "PATH=$PATH"
  exit 2
fi

if ! command -v bc >/dev/null 2>&1; then
  echo "bc não encontrado no PATH" >&2
  incident_log_append "check_queue.sh" "error" "bc ausente" "PATH=$PATH"
  exit 2
fi

printf '%s\n' "═══════════════════════════════════════"
printf '  VVTV QUEUE STATUS\n'
printf '%s\n' "═══════════════════════════════════════"
printf 'Time: %s\n\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

printf '📊 Queue Overview:\n'
sqlite3 "$QUEUE_DB" <<'SQL_OVERVIEW'
.mode column
.headers on
SELECT
    status,
    COUNT(*) AS count,
    ROUND(SUM(duration_s)/3600.0, 2) AS hours,
    ROUND(AVG(curation_score), 2) AS avg_score
FROM playout_queue
GROUP BY status
ORDER BY
    CASE status
        WHEN 'queued' THEN 1
        WHEN 'playing' THEN 2
        WHEN 'played' THEN 3
        WHEN 'failed' THEN 4
        ELSE 5
    END;
SQL_OVERVIEW

printf '\n🎬 Recent Queued Items (limit %s):\n' "$RECENT"
sqlite3 "$QUEUE_DB" <<SQL_RECENT
.mode column
.headers on
SELECT
    SUBSTR(plan_id, 1, 8) AS plan,
    SUBSTR(asset_path, -30) AS asset,
    ROUND(duration_s/60.0, 1) AS mins,
    ROUND(curation_score, 2) AS score,
    priority,
    created_at
FROM playout_queue
WHERE status='queued'
ORDER BY created_at ASC
LIMIT $RECENT;
SQL_RECENT

total_seconds=$(sqlite3 "$QUEUE_DB" "SELECT COALESCE(SUM(duration_s),0) FROM playout_queue WHERE status='queued';")
BUFFER_H=$(echo "scale=2; $total_seconds / 3600" | bc)

printf '\n⏱️  Buffer Analysis:\n'
printf 'Total queued: %sh\n' "$BUFFER_H"

STATUS="healthy"
EXIT_CODE=0

if (( $(echo "$BUFFER_H < 2" | bc -l) )); then
  printf '🔴 CRITICAL: Buffer below 2h!\n'
  STATUS="critical"
  EXIT_CODE=2
elif (( $(echo "$BUFFER_H < 3" | bc -l) )); then
  printf '🟡 WARNING: Buffer below 3h\n'
  STATUS="warning"
  EXIT_CODE=1
else
  printf '✅ Buffer healthy (>3h)\n'
fi

incident_log_append "check_queue.sh" "$STATUS" "Buffer ${BUFFER_H}h" "recent=$RECENT"

if [[ $OUTPUT_JSON -eq 1 ]]; then
  python3 - <<PY
import json
print(json.dumps({
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "buffer_hours": float("$BUFFER_H"),
    "status": "$STATUS",
    "recent_limit": int("$RECENT")
}))
PY
fi

exit $EXIT_CODE
