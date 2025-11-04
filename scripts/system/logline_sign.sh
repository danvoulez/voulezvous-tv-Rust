#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --key <pem_path> --file <target> [--vault-dir <path>] [--note <text>]

Assina arquivos críticos utilizando logline (ou fallback sha256) e
armazena o manifesto no vault configurado.
USAGE
  exit 1
}

KEY=""
FILE=""
VAULT_DIR="/vvtv/vault"
NOTE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --key)
      KEY="$2"
      shift 2
      ;;
    --file)
      FILE="$2"
      shift 2
      ;;
    --vault-dir)
      VAULT_DIR="$2"
      shift 2
      ;;
    --note)
      NOTE="$2"
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

if [[ -z "$KEY" || -z "$FILE" ]]; then
  echo "--key e --file são obrigatórios" >&2
  usage
fi

if [[ ! -f "$FILE" ]]; then
  echo "Arquivo alvo $FILE não encontrado" >&2
  exit 1
fi

mkdir -p "$VAULT_DIR/manifests"
TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
BASENAME=$(basename "$FILE")
MANIFEST="$VAULT_DIR/manifests/${BASENAME}.${TIMESTAMP}.logline"

log() {
  echo "[$(date --iso-8601=seconds 2>/dev/null || date)] $*"
}

if command -v logline >/dev/null 2>&1; then
  log "[INFO] Usando logline nativo para assinar"
  logline sign --key "$KEY" --note "$NOTE" --output "$MANIFEST" "$FILE"
  chmod 600 "$MANIFEST"
else
  log "[WARN] logline não encontrado. Usando fallback sha256."
  SHA=$(sha256sum "$FILE" | awk '{print $1}')
  cat <<EOF_MANIFEST > "$MANIFEST"
logline-fallback-version = 1
file = "$FILE"
sha256 = "$SHA"
note = "$NOTE"
timestamp = "$TIMESTAMP"
EOF_MANIFEST
  chmod 600 "$MANIFEST"
fi

log "Manifesto registrado em $MANIFEST"
