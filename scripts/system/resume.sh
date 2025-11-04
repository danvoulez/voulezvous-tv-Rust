#!/bin/bash
# VVTV resume ritual

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
LOG_FILE=${VVTV_STANDBY_LOG:-$VVTV_BASE_DIR/system/logs/standby.log}
SNAPSHOT_DIR=${VVTV_STANDBY_DIR:-$VVTV_BASE_DIR/vault/snapshots}
LATEST_SNAPSHOT=$(ls -1t "$SNAPSHOT_DIR"/standby_*.tar.zst 2>/dev/null | head -n1)

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [resume] $*" | tee -a "$LOG_FILE"
}

if [[ -z "$LATEST_SNAPSHOT" ]]; then
    log "Nenhum snapshot standby encontrado"
    exit 1
fi

log "Restaurando snapshot $LATEST_SNAPSHOT"
if command -v zstd >/dev/null 2>&1 && command -v tar >/dev/null 2>&1; then
    zstd -d <"$LATEST_SNAPSHOT" | tar -C "$VVTV_BASE_DIR" -xf -
else
    log "zstd ou tar indisponível"
fi

if command -v logline >/dev/null 2>&1; then
    logline revive "$LATEST_SNAPSHOT" >>"$LOG_FILE" 2>&1 || log "logline revive retornou código $?"
fi

if command -v systemctl >/dev/null 2>&1; then
    for svc in vvtv-processor.service vvtv-broadcaster.service vvtv-curator.service; do
        sudo systemctl start "$svc" >/dev/null 2>&1 || true
    done
fi

log "Resumed from standby"
