#!/bin/bash
# VVTV standby ritual

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
LOG_FILE=${VVTV_STANDBY_LOG:-$VVTV_BASE_DIR/system/logs/standby.log}
SNAPSHOT_DIR=${VVTV_STANDBY_DIR:-$VVTV_BASE_DIR/vault/snapshots}
FINAL_FRAME=${VVTV_FINAL_FRAME:-$VVTV_BASE_DIR/vault/final_frame.jpg}

mkdir -p "$(dirname "$LOG_FILE")" "$SNAPSHOT_DIR"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [standby] $*" | tee -a "$LOG_FILE"
}

log "Iniciando ritual de standby"

if command -v systemctl >/dev/null 2>&1; then
    for svc in vvtv-broadcaster.service vvtv-processor.service vvtv-curator.service; do
        sudo systemctl stop "$svc" >/dev/null 2>&1 || true
    done
fi

if command -v logline >/dev/null 2>&1; then
    logline shutdown --ritual=vvtv >>"$LOG_FILE" 2>&1 || log "logline shutdown retornou código $?"
fi

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
ARCHIVE="$SNAPSHOT_DIR/standby_$STAMP.tar.zst"
if command -v tar >/dev/null 2>&1 && command -v zstd >/dev/null 2>&1; then
    log "Compactando snapshot operacional"
    tar -C "$VVTV_BASE_DIR" -cf - data storage broadcast configs 2>/dev/null | zstd -T0 -19 -o "$ARCHIVE"
    log "Snapshot salvo em $ARCHIVE"
fi

if command -v ffmpeg >/dev/null 2>&1; then
    log "Capturando frame final"
    ffmpeg -y -i "$VVTV_BASE_DIR/broadcast/hls/live.m3u8" -frames:v 1 "$FINAL_FRAME" >/dev/null 2>&1 || log "Falha ao capturar frame"
fi

log "Standby concluído"
