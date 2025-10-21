#!/bin/bash
# VVTV Backup Restore Tester

set -euo pipefail

ARCHIVE=${1:-}
TARGET_DIR=${2:-/tmp/vvtv_restore_test}
LOG_FILE=${VVTV_RESTORE_LOG:-/vvtv/system/logs/restore_test.log}

mkdir -p "$(dirname "$LOG_FILE")"

die() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [ERROR] $*" | tee -a "$LOG_FILE"
    exit 1
}

if [[ -z "$ARCHIVE" ]]; then
    die "Uso: $0 <arquivo .tar.zst> [destino]"
fi

if [[ ! -f "$ARCHIVE" ]]; then
    die "Arquivo $ARCHIVE não encontrado"
fi

echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] Testando restauração de $ARCHIVE" | tee -a "$LOG_FILE"
rm -rf "$TARGET_DIR"
mkdir -p "$TARGET_DIR"

if command -v zstd >/dev/null 2>&1 && command -v tar >/dev/null 2>&1; then
    zstd -d <"$ARCHIVE" | tar -C "$TARGET_DIR" -xf -
else
    die "Requer tar e zstd instalados"
fi

echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] Verificando integridade dos bancos" | tee -a "$LOG_FILE"
find "$TARGET_DIR" -name '*.sqlite' -print0 | while IFS= read -r -d '' db; do
    if command -v sqlite3 >/dev/null 2>&1; then
        if sqlite3 "$db" 'PRAGMA integrity_check;' | grep -qv ok; then
            echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [WARN] Integrity check falhou para $db" | tee -a "$LOG_FILE"
        fi
    fi
done

echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] Restauração concluída em $TARGET_DIR" | tee -a "$LOG_FILE"
