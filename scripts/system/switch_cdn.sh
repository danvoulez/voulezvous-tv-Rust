#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

if [[ $# -lt 1 ]]; then
  cat <<USAGE
Usage: $0 <hostname> [--reason <motivo>]
Altera o registro DNS da CDN primária via Cloudflare API.
Variáveis esperadas:
  CLOUDFLARE_API_TOKEN
  CLOUDFLARE_ZONE_ID
  CLOUDFLARE_RECORD_ID
  CLOUDFLARE_RECORD_NAME (padrão: voulezvous.tv)
  CDN_LOG (padrão: /vvtv/system/logs/cdn_failover.log)
USAGE
  exit 1
fi

TARGET_HOST="$1"
shift
REASON="manual"
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --reason)
      REASON="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    *)
      echo "Opção desconhecida: $1" >&2
      exit 1
      ;;
  esac
done

: "${CLOUDFLARE_API_TOKEN:?CLOUDFLARE_API_TOKEN não definido}"
: "${CLOUDFLARE_ZONE_ID:?CLOUDFLARE_ZONE_ID não definido}"
: "${CLOUDFLARE_RECORD_ID:?CLOUDFLARE_RECORD_ID não definido}"
RECORD_NAME=${CLOUDFLARE_RECORD_NAME:-"voulezvous.tv"}
LOG_FILE=${CDN_LOG:-"/vvtv/system/logs/cdn_failover.log"}

mkdir -p "$(dirname "$LOG_FILE")"
TIMESTAMP=$(date --iso-8601=seconds 2>/dev/null || date)

if [[ $DRY_RUN -eq 1 ]]; then
  echo "[DRY-RUN] Atualizaria DNS $RECORD_NAME -> $TARGET_HOST" >&2
  echo "{\"timestamp\":\"$TIMESTAMP\",\"target\":\"$TARGET_HOST\",\"reason\":\"$REASON\",\"status\":\"dry-run\"}" >>"$LOG_FILE"
  incident_log_append "switch_cdn.sh" "dry-run" "simulado para $TARGET_HOST" "reason=$REASON"
  exit 0
fi

API="https://api.cloudflare.com/client/v4/zones/$CLOUDFLARE_ZONE_ID/dns_records/$CLOUDFLARE_RECORD_ID"
PAYLOAD=$(cat <<JSON
{
  "type": "CNAME",
  "name": "$RECORD_NAME",
  "content": "$TARGET_HOST",
  "ttl": 120,
  "proxied": true
}
JSON
)

RESPONSE=$(curl -fsS -X PATCH "$API" \
  -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
  -H "Content-Type: application/json" \
  --data "$PAYLOAD")

STATUS=$(echo "$RESPONSE" | grep -q '"success":true' && echo success || echo error)

echo "{\"timestamp\":\"$TIMESTAMP\",\"target\":\"$TARGET_HOST\",\"reason\":\"$REASON\",\"status\":\"$STATUS\"}" >>"$LOG_FILE"

if [[ $STATUS != "success" ]]; then
  echo "[ERROR] Falha ao atualizar DNS: $RESPONSE" >&2
  incident_log_append "switch_cdn.sh" "error" "falha ao apontar" "target=$TARGET_HOST reason=$REASON"
  exit 1
fi

echo "[INFO] CDN apontada para $TARGET_HOST" >&2
incident_log_append "switch_cdn.sh" "success" "cdn apontada" "target=$TARGET_HOST reason=$REASON"
