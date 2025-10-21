#!/bin/bash
# VVTV Cold Backup (daily full snapshot)

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/lib/backup_common.sh"

LOG_FILE=${VVTV_BACKUP_LOG_COLD:-$VVTV_LOG_DIR/backup_cold.log}
mkdir -p "$(dirname "$LOG_FILE")"

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
SNAPSHOT_ROOT="$VVTV_BACKUP_DIR/cold"
ARCHIVE_PATH="$SNAPSHOT_ROOT/vvtv_cold_${TIMESTAMP}.tar.zst"
MANIFEST_PATH="$SNAPSHOT_ROOT/vvtv_cold_${TIMESTAMP}.json"
REMOTE_COLD=${VVTV_BACKUP_REMOTE_COLD:-}
REMOTE_FLAGS=${VVTV_BACKUP_REMOTE_FLAGS:---fast-list}

log INFO "Iniciando backup COLD"
mkdir -p "$SNAPSHOT_ROOT"

TMPDIR=$(mktemp -d "$SNAPSHOT_ROOT/tmp.XXXXXX")
trap 'rm -rf "$TMPDIR"' EXIT

log INFO "Copiando diretórios principais para staging"
rsync_copy "$VVTV_STORAGE_DIR/" "$TMPDIR/storage" "storage"
rsync_copy "$VVTV_DATA_DIR/" "$TMPDIR/data" "data"
rsync_copy "$VVTV_BROADCAST_DIR/" "$TMPDIR/broadcast" "broadcast"

# Incluir vault metadata (sem chaves privadas)
if [ -d "$VVTV_BASE_DIR/vault/manifests" ]; then
    rsync_copy "$VVTV_BASE_DIR/vault/manifests/" "$TMPDIR/vault/manifests" "vault/manifests"
fi

# Gerar manifest JSON
cat >"$MANIFEST_PATH" <<JSON
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "hostname": "$(hostname -s)",
  "node_id": "${VVTV_NODE_ID:-unknown}",
  "paths": {
    "storage": "$VVTV_STORAGE_DIR",
    "data": "$VVTV_DATA_DIR",
    "broadcast": "$VVTV_BROADCAST_DIR"
  },
  "archive": "$(basename "$ARCHIVE_PATH")"
}
JSON

if command -v logline >/dev/null 2>&1 && [ -n "${VVTV_LOGLINE_KEY:-}" ]; then
    log INFO "Assinando manifest com logline"
    LOG_LINE_CMD=(logline sign --key "$VVTV_LOGLINE_KEY" "$MANIFEST_PATH")
    if ! "${LOG_LINE_CMD[@]}" >>"$LOG_FILE" 2>&1; then
        log WARN "Falha ao assinar manifest"
    fi
fi

log INFO "Compactando snapshot completo"
compress_tar "$TMPDIR" "$ARCHIVE_PATH" "cold-$TIMESTAMP"

if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$ARCHIVE_PATH" >"$ARCHIVE_PATH.sha256"
fi

if rclone_sync "$ARCHIVE_PATH" "$REMOTE_COLD" "$REMOTE_FLAGS"; then
    rclone_check "$ARCHIVE_PATH" "$REMOTE_COLD" "$REMOTE_FLAGS" || true
    if [ -f "$ARCHIVE_PATH.sha256" ]; then
        rclone_sync "$ARCHIVE_PATH.sha256" "$REMOTE_COLD" "$REMOTE_FLAGS" || true
    fi
    rclone_sync "$MANIFEST_PATH" "$REMOTE_COLD" "$REMOTE_FLAGS" || true
fi

rm -rf "$TMPDIR"
rotate_backups "$SNAPSHOT_ROOT" $((30 * 24 * 3600))

log INFO "Backup COLD concluído"
