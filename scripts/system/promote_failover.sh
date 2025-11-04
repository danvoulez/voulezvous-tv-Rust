#!/usr/bin/env bash
set -euo pipefail

REASON="manual"
PERCENT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --reason)
      REASON="$2"
      shift 2
      ;;
    --percent)
      PERCENT="$2"
      shift 2
      ;;
    -h|--help)
      cat <<USAGE
Usage: $0 [--reason <motivo>] [--percent <drift>]
Dispara promoção do origin secundário (Railway) e registra auditoria.
USAGE
      exit 0
      ;;
    *)
      echo "Opção desconhecida: $1" >&2
      exit 1
      ;;
  esac
done

LOG_FILE=${FAILOVER_LOG:-"/vvtv/system/logs/failover.log"}
mkdir -p "$(dirname "$LOG_FILE")"
TIMESTAMP=$(date --iso-8601=seconds 2>/dev/null || date)
STATUS="triggered"

if command -v systemctl >/dev/null 2>&1; then
  if ! systemctl start vvtv-failover.target >/dev/null 2>&1; then
    STATUS="systemctl_failed"
  fi
else
  STATUS="systemctl_unavailable"
fi

echo "{\"timestamp\":\"$TIMESTAMP\",\"reason\":\"$REASON\",\"drift_percent\":\"$PERCENT\",\"status\":\"$STATUS\"}" >>"$LOG_FILE"

echo "[INFO] Failover acionado (motivo=$REASON, drift=$PERCENT, status=$STATUS)" >&2
