#!/usr/bin/env bash
# VVTV Encoder Restart — gracefully restarts ffmpeg broadcaster

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
LOG_FILE=${BROADCAST_LOG:-"$VVTV_BASE_DIR/system/logs/broadcast.log"}
SERVICE_NAME=${SERVICE_NAME:-vvtv_broadcast}
DRY_RUN=0

usage() {
  cat <<'USAGE'
Usage: $(basename "$0") [--dry-run]

Options:
  --dry-run   Não envia sinais nem reinicia serviços (apenas simula)
  --help      Exibe esta mensagem
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
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

log "🔄 Encoder restart requested (dry-run=$DRY_RUN)"

PIDS=$(pgrep -f "ffmpeg.*rtmp" || true)
if [[ -n "$PIDS" ]]; then
  log "🛑 Stopping PIDs: $PIDS"
  if [[ $DRY_RUN -eq 0 ]]; then
    kill -SIGTERM $PIDS || true
    sleep 3
    REMAINING=$(pgrep -f "ffmpeg.*rtmp" || true)
    if [[ -n "$REMAINING" ]]; then
      log "⚠️  Force killing: $REMAINING"
      kill -SIGKILL $REMAINING || true
    fi
  fi
else
  log "ℹ️ Nenhum processo ffmpeg ativo"
fi

restart_status="skipped"
if [[ $DRY_RUN -eq 0 ]]; then
  if command -v systemctl >/dev/null 2>&1; then
    log "📢 Restarting via systemd ($SERVICE_NAME)"
    if systemctl restart "$SERVICE_NAME"; then
      restart_status="systemd"
    else
      log "⚠️  Falha ao reiniciar via systemd"
      restart_status="systemd-failed"
    fi
  else
    log "📢 Sistema sem systemd — execute script manual"
    restart_status="manual-required"
  fi
else
  log "[DRY-RUN] Pulando reinício via systemd"
  restart_status="dry-run"
fi

sleep 2

if pgrep -f "ffmpeg.*rtmp" >/dev/null 2>&1; then
  log "✅ Encoder ativo após reinício"
  incident_log_append "restart_encoder.sh" "success" "encoder reiniciado" "mode=$restart_status"
  exit 0
fi

if [[ $DRY_RUN -eq 1 ]]; then
  log "⚠️  Dry-run concluído sem validar processos"
  incident_log_append "restart_encoder.sh" "dry-run" "simulação concluída" "mode=$restart_status"
  exit 0
fi

log "❌ Encoder não iniciou após tentativa"
incident_log_append "restart_encoder.sh" "error" "encoder indisponível" "mode=$restart_status"
exit 1
