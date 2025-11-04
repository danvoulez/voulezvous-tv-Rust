#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --manifest <manifest.logline> [--file <target>]

Valida assinaturas geradas por logline_sign.sh ou logline nativo.
USAGE
  exit 1
}

MANIFEST=""
FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest)
      MANIFEST="$2"
      shift 2
      ;;
    --file)
      FILE="$2"
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

if [[ -z "$MANIFEST" ]]; then
  echo "--manifest é obrigatório" >&2
  usage
fi

if [[ ! -f "$MANIFEST" ]]; then
  echo "Manifesto $MANIFEST não encontrado" >&2
  exit 1
fi

log() {
  echo "[$(date --iso-8601=seconds 2>/dev/null || date)] $*"
}

if command -v logline >/dev/null 2>&1; then
  log "[INFO] Usando logline nativo para verificação"
  if [[ -n "$FILE" ]]; then
    logline verify --manifest "$MANIFEST" --file "$FILE"
  else
    logline verify --manifest "$MANIFEST"
  fi
  exit 0
fi

log "[INFO] logline não encontrado. Validando fallback sha256"

if [[ -z "$FILE" ]]; then
  FILE=$(grep '^file = ' "$MANIFEST" | cut -d'"' -f2)
fi

if [[ ! -f "$FILE" ]]; then
  echo "Arquivo alvo $FILE não encontrado para verificação" >&2
  exit 1
fi

EXPECTED=$(grep '^sha256 = ' "$MANIFEST" | awk -F'"' '{print $2}')
CURRENT=$(sha256sum "$FILE" | awk '{print $1}')

if [[ "$EXPECTED" == "$CURRENT" ]]; then
  log "[OK] Assinatura confere"
else
  log "[FAIL] Assinatura divergente"
  exit 2
fi
