#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--quiet] [--dry-run]

Realiza verificações básicas do pipeline de broadcast:
- Processo ffmpeg ativo
- Segmentos HLS recentes
- Status do NGINX
- Métricas SQLite
USAGE
  exit 1
}

QUIET=false
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quiet)
      QUIET=true
      shift
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
  if ! $QUIET; then
    echo "[$(date --iso-8601=seconds 2>/dev/null || date)] $*"
  fi
}

STATUS=0
LOG_FILE="/vvtv/system/logs/health_checks.log"
mkdir -p /vvtv/system/logs >/dev/null 2>&1 || true
exec 3>>"$LOG_FILE"

report() {
  local level="$1"; shift
  local message="$*"
  local line="[$(date --iso-8601=seconds 2>/dev/null || date)] [$level] $message"
  echo "$line" >&3
  log "[$level] $message"
  [[ "$level" == "ERROR" ]] && STATUS=1
}

if pgrep -f "ffmpeg.*rtmp" >/dev/null 2>&1; then
  report INFO "Processo FFmpeg ativo"
else
  report ERROR "Processo FFmpeg não encontrado"
fi

HLS_DIR="/vvtv/broadcast/hls"
if [[ -d $HLS_DIR ]]; then
  latest_segment=$(find "$HLS_DIR" -type f -name '*.ts' -printf '%T@ %p\n' 2>/dev/null | sort -nr | head -n1 | awk '{print $1}')
  now=$(date +%s)
  if [[ -n "$latest_segment" ]]; then
    age=$(printf '%.0f' "$(echo "$now - $latest_segment" | bc -l 2>/dev/null || echo 9999)")
    if [[ $age -le 30 ]]; then
      report INFO "Segmentos HLS atualizados (último há ${age}s)"
    else
      report ERROR "Sem segmentos HLS recentes (último há ${age}s)"
    fi
  else
    report ERROR "Nenhum segmento HLS encontrado"
  fi
else
  report ERROR "Diretório HLS ausente"
fi

if command -v curl >/dev/null 2>&1; then
  if curl -fsS http://localhost:8080/status >/dev/null 2>&1; then
    report INFO "Endpoint /status respondeu"
  else
    report ERROR "Falha ao consultar http://localhost:8080/status"
  fi
fi

if command -v sqlite3 >/dev/null 2>&1 && [[ -f /vvtv/data/metrics.sqlite ]]; then
  buffer=$(sqlite3 /vvtv/data/metrics.sqlite "SELECT buffer_duration_h FROM metrics ORDER BY ts DESC LIMIT 1;" 2>/dev/null || echo "")
  if [[ -n "$buffer" ]]; then
    report INFO "Buffer atual registrado: ${buffer}h"
  else
    report INFO "Sem métricas recentes registradas"
  fi
fi

if $DRY_RUN; then
  report INFO "Execução em modo dry-run, nenhuma ação corretiva realizada"
fi

exit $STATUS
