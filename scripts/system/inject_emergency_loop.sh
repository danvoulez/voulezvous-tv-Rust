#!/usr/bin/env bash
# VVTV Emergency Loop Injector — add safe content when the buffer is low

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
ARCHIVE_DIR=${ARCHIVE_DIR:-"$VVTV_BASE_DIR/storage/archive"}
QUEUE_DB=${QUEUE_DB:-"$VVTV_BASE_DIR/data/queue.sqlite"}
LOG_FILE=${EMERGENCY_LOG:-"$VVTV_BASE_DIR/system/logs/emergency.log"}
DRY_RUN=0
SAFE_COUNT=5

usage() {
  cat <<'USAGE'
Usage: $(basename "$0") [--count N] [--dry-run]

Options:
  --count N   Número máximo de itens seguros a injetar (padrão: 5)
  --dry-run   Não altera bancos de dados; apenas mostra ações
  --help      Exibe esta mensagem
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --count)
      SAFE_COUNT="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
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

log() {
  local message="$1"
  mkdir -p "$(dirname "$LOG_FILE")"
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $message" | tee -a "$LOG_FILE"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "❌ Comando obrigatório ausente: $1"
    incident_log_append "inject_emergency_loop.sh" "error" "comando ausente" "$1"
    exit 2
  fi
}

require_command sqlite3
require_command find
require_command shuf
require_command ffprobe
require_command uuidgen
require_command bc

if [[ ! -d "$ARCHIVE_DIR" ]]; then
  log "❌ Arquivo de emergência não encontrado em $ARCHIVE_DIR"
  incident_log_append "inject_emergency_loop.sh" "error" "arquivo de emergência ausente" "$ARCHIVE_DIR"
  exit 2
fi

if [[ ! -f "$QUEUE_DB" ]]; then
  log "❌ queue.sqlite não encontrado em $QUEUE_DB"
  incident_log_append "inject_emergency_loop.sh" "error" "queue.sqlite ausente" "$QUEUE_DB"
  exit 2
fi

log "🚨 EMERGENCY LOOP ACTIVATION (dry-run=$DRY_RUN)"

SAFE_CONTENT=$(find "$ARCHIVE_DIR" -type f -name '*.mp4' -mtime -30 2>/dev/null | shuf -n "$SAFE_COUNT" || true)
COUNT=$(printf '%s\n' "$SAFE_CONTENT" | sed '/^$/d' | wc -l | tr -d ' ')

if [[ -z "$SAFE_CONTENT" ]]; then
  log "❌ Nenhum conteúdo seguro encontrado (últimos 30 dias)"
  incident_log_append "inject_emergency_loop.sh" "error" "sem conteúdo seguro" "count=0"
  exit 1
fi

log "Found $COUNT safe items"

sql_escape() {
  printf "%s" "$1" | sed "s/'/''/g"
}

total_added=0
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  PLAN_ID="emergency-$(uuidgen)"
  DURATION_RAW=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$file" | head -n1 || echo 0)
  if [[ -z "$DURATION_RAW" ]]; then
    DURATION_RAW=0
  fi
  DURATION=$(python3 - <<PY
try:
    import math
    print(int(math.ceil(float("$DURATION_RAW"))))
except Exception:
    print(0)
PY
)
  (( total_added++ ))

  if [[ $DRY_RUN -eq 0 ]]; then
    sqlite3 "$QUEUE_DB" "INSERT INTO playout_queue (plan_id, asset_path, duration_s, status, priority, node_origin) VALUES ('$(sql_escape "$PLAN_ID")', '$(sql_escape "$file")', $DURATION, 'queued', 1, 'emergency-loop');"
  fi

  log "✅ Injected: $(basename "$file") (${DURATION}s)"
done <<< "$SAFE_CONTENT"

total_seconds=$(sqlite3 "$QUEUE_DB" "SELECT COALESCE(SUM(duration_s),0) FROM playout_queue WHERE status='queued';")
BUFFER_H=$(echo "scale=2; $total_seconds / 3600" | bc)

log "📊 New buffer: ${BUFFER_H}h"
log "🔄 Emergency loop complete"

status="success"
context="items=$total_added buffer=${BUFFER_H}h dry_run=$DRY_RUN"
incident_log_append "inject_emergency_loop.sh" "$status" "loop executado" "$context"

if command -v telegram-send >/dev/null 2>&1 && [[ $DRY_RUN -eq 0 ]]; then
  telegram-send "🚨 VVTV: Emergency loop ativado. Buffer agora: ${BUFFER_H}h (${total_added} itens)"
fi
