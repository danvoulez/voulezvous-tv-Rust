#!/bin/bash
# VVTV LogLine identity rotation utility

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
KEY_PATH=${VVTV_LOGLINE_KEY_PATH:-$VVTV_BASE_DIR/vault/keys/voulezvous_foundation.pem}
BACKUP_DIR=${VVTV_KEY_BACKUP_DIR:-$VVTV_BASE_DIR/vault/keys/archive}
LOG_FILE=${VVTV_SECURITY_LOG:-$VVTV_BASE_DIR/system/logs/security.log}
CONFIG_DIR=${VVTV_CONFIG_DIR:-$VVTV_BASE_DIR/system}

mkdir -p "$(dirname "$LOG_FILE")" "$BACKUP_DIR"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [rotate-keys] $*" | tee -a "$LOG_FILE"
}

if [[ ! -f "$KEY_PATH" ]]; then
    log "Erro: chave atual não encontrada em $KEY_PATH"
    exit 1
fi

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
BACKUP_KEY="$BACKUP_DIR/voulezvous_foundation_$STAMP.pem"
cp "$KEY_PATH" "$BACKUP_KEY"
chmod 600 "$BACKUP_KEY"
log "Backup criado: $BACKUP_KEY"

if command -v openssl >/dev/null 2>&1; then
    NEW_KEY="$KEY_PATH.new"
    openssl genrsa -out "$NEW_KEY" 4096 >/dev/null 2>&1
    chmod 600 "$NEW_KEY"
    mv "$NEW_KEY" "$KEY_PATH"
    log "Nova chave RSA 4096 bits gerada"
else
    log "openssl indisponível, mantendo chave anterior"
fi

if command -v logline >/dev/null 2>&1; then
    for target in "$VVTV_BASE_DIR/configs/vvtv.toml" \
        "$VVTV_BASE_DIR/configs/broadcaster.toml" \
        "$VVTV_BASE_DIR/configs/processor.toml" \
        "$VVTV_BASE_DIR/configs/browser.toml"; do
        if [[ -f "$target" ]]; then
            log "Assinando $target"
            logline sign --key "$KEY_PATH" "$target" >>"$LOG_FILE" 2>&1 || log "Falha ao assinar $target"
        fi
    done
else
    log "logline não encontrado, assinatura pulada"
fi

log "Rotação de chaves concluída"
