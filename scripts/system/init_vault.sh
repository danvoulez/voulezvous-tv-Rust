#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--vault-dir <path>] [--key <pem_path>] [--force]

Inicializa a estrutura computável do vault VVTV.
Cria subdiretórios snapshots/, keys/ e manifests/ com permissões 700.
Se uma chave PEM for informada, ajusta permissões para 600.
USAGE
  exit 1
}

VAULT_DIR="/vvtv/vault"
KEY_PATH=""
FORCE=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --vault-dir)
      VAULT_DIR="$2"
      shift 2
      ;;
    --key)
      KEY_PATH="$2"
      shift 2
      ;;
    --force)
      FORCE=true
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
  echo "[$(date --iso-8601=seconds 2>/dev/null || date)] $*"
}

if [[ -d "$VAULT_DIR" && $FORCE == false ]]; then
  log "[INFO] Vault já existe em $VAULT_DIR. Use --force para reconfigurar permissões."
else
  mkdir -p "$VAULT_DIR"
fi

for sub in snapshots keys manifests; do
  mkdir -p "$VAULT_DIR/$sub"
  chmod 700 "$VAULT_DIR/$sub"
  log "[OK] Diretório $VAULT_DIR/$sub pronto"
done

chmod 700 "$VAULT_DIR"

if [[ -n "$KEY_PATH" ]]; then
  if [[ ! -f "$KEY_PATH" ]]; then
    echo "[ERRO] Chave $KEY_PATH não encontrada" >&2
    exit 1
  fi
  chmod 600 "$KEY_PATH"
  log "[OK] Permissões da chave ajustadas para 600"
fi

log "Vault configurado com sucesso"
