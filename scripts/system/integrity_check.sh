#!/usr/bin/env bash
# VVTV Integrity Check — verifies databases, filesystem and core services

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
REPORT_DIR=${REPORT_DIR:-"$VVTV_BASE_DIR/system/reports"}
LOG_FILE=${INTEGRITY_LOG:-"$VVTV_BASE_DIR/system/logs/integrity_check.log"}
REPORT_FILE=""

usage() {
  cat <<'USAGE'
Usage: $(basename "$0") [--report PATH]

Options:
  --report PATH  Salva o relatório JSON no caminho especificado
  --help         Exibe esta mensagem
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --report)
      REPORT_FILE="$2"
      shift 2
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

mkdir -p "$REPORT_DIR" "$(dirname "$LOG_FILE")"
if [[ -z "$REPORT_FILE" ]]; then
  REPORT_FILE="$REPORT_DIR/integrity_$(date -u +%Y%m%dT%H%M%SZ).json"
fi

log() {
  local message="$1"
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $message" | tee -a "$LOG_FILE"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "❌ Comando obrigatório ausente: $1"
    incident_log_append "integrity_check.sh" "error" "comando ausente" "$1"
    exit 2
  fi
}

json_array() {
  if [[ $# -eq 0 ]]; then
    echo "[]"
    return
  fi
  python3 - "$@" <<'PY'
import json, sys
print(json.dumps(sys.argv[1:]))
PY
}

require_command sqlite3

CHECKS_PASSED=0
CHECKS_FAILED=0
ISSUES=()

check() {
  local name="$1"
  local command="$2"

  if eval "$command" >>"$LOG_FILE" 2>&1; then
    log "✅ $name"
    ((CHECKS_PASSED++))
    return 0
  else
    log "❌ $name"
    ISSUES+=("$name")
    ((CHECKS_FAILED++))
    return 1
  fi
}

DATA_DIR="$VVTV_BASE_DIR/data"
BROADCAST_DIR="$VVTV_BASE_DIR/broadcast"

check "plans.sqlite integrity" \
  "sqlite3 $DATA_DIR/plans.sqlite 'PRAGMA integrity_check;' | grep -q '^ok$'"

check "queue.sqlite integrity" \
  "sqlite3 $DATA_DIR/queue.sqlite 'PRAGMA integrity_check;' | grep -q '^ok$'"

DISK_USAGE=$(df "$VVTV_BASE_DIR" 2>/dev/null | tail -1 | awk '{print $5}' | tr -d '%')
if [[ -z "$DISK_USAGE" ]]; then
  DISK_USAGE=0
fi
if (( DISK_USAGE < 80 )); then
  log "✅ Disk usage: ${DISK_USAGE}%"
  ((CHECKS_PASSED++))
else
  log "❌ Disk usage crítico: ${DISK_USAGE}%"
  ISSUES+=("Disk usage ${DISK_USAGE}%")
  ((CHECKS_FAILED++))
fi

CPU_TEMP=""
if command -v osx-cpu-temp >/dev/null 2>&1; then
  CPU_TEMP=$(osx-cpu-temp -c | cut -d'°' -f1)
elif [[ -f /sys/class/thermal/thermal_zone0/temp ]]; then
  CPU_TEMP=$(awk '{print $1/1000}' /sys/class/thermal/thermal_zone0/temp)
fi
if [[ -n "$CPU_TEMP" ]]; then
  if command -v bc >/dev/null 2>&1 && (( $(echo "$CPU_TEMP < 75" | bc -l) )); then
    log "✅ CPU temp: ${CPU_TEMP}°C"
    ((CHECKS_PASSED++))
  elif command -v bc >/dev/null 2>&1; then
    log "❌ CPU temp alta: ${CPU_TEMP}°C"
    ISSUES+=("CPU temp ${CPU_TEMP}°C")
    ((CHECKS_FAILED++))
  else
    log "⚠️  Não foi possível avaliar temperatura (bc ausente)"
  fi
fi

check "NGINX running" "pgrep -f nginx"
check "Tailscale running" "tailscale status"
check "HLS playlist exists" "test -f $BROADCAST_DIR/hls/live.m3u8"
check "Clock synchronized" "timedatectl status 2>/dev/null | grep -qi 'synchronized: yes' || (which ntpdate >/dev/null 2>&1 && ntpdate -q pool.ntp.org >/dev/null)"

ISSUES_JSON=$(json_array "${ISSUES[@]}")

cat >"$REPORT_FILE" <<EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "checks_passed": $CHECKS_PASSED,
  "checks_failed": $CHECKS_FAILED,
  "disk_usage_percent": $DISK_USAGE,
  "issues": $ISSUES_JSON
}
EOF

log "📊 Report saved: $REPORT_FILE"
log "Summary: $CHECKS_PASSED passed, $CHECKS_FAILED failed"

STATUS="success"
EXIT_CODE=0
if [[ $CHECKS_FAILED -gt 0 ]]; then
  STATUS="warning"
  EXIT_CODE=1
fi

incident_log_append "integrity_check.sh" "$STATUS" "integrity summary" "passed=$CHECKS_PASSED failed=$CHECKS_FAILED"
exit $EXIT_CODE
