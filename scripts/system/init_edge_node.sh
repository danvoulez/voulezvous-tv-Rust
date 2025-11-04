#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --origin <url> --region <região>
Inicializa um nó edge com cache local de 15 s e probes de latência.
USAGE
  exit 1
}

ORIGIN=""
REGION=""
PROFILE_DIR=${EDGE_PROFILE_DIR:-"/vvtv/cache/edge"}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --origin)
      ORIGIN="$2"
      shift 2
      ;;
    --region)
      REGION="$2"
      shift 2
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

[[ -n "$ORIGIN" && -n "$REGION" ]] || usage

mkdir -p "$PROFILE_DIR"

LOG_FILE=${EDGE_LOG:-"/vvtv/system/logs/edge_nodes.log"}
mkdir -p "$(dirname "$LOG_FILE")"
TIMESTAMP=$(date --iso-8601=seconds 2>/dev/null || date)

if command -v logline >/dev/null 2>&1; then
  logline --init-node --role=edge --origin="$ORIGIN" --region="$REGION" --profile-dir="$PROFILE_DIR"
  STATUS=$?
else
  STATUS=127
fi

echo "{\"timestamp\":\"$TIMESTAMP\",\"origin\":\"$ORIGIN\",\"region\":\"$REGION\",\"status\":$STATUS}" >>"$LOG_FILE"

exit $STATUS
