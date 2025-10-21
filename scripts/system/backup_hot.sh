#!/bin/bash
# VVTV Hot Backup (hourly, nearline copy)

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/lib/backup_common.sh"

LOG_FILE=${VVTV_BACKUP_LOG_HOT:-$VVTV_LOG_DIR/backup_hot.log}
mkdir -p "$(dirname "$LOG_FILE")"

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
SNAPSHOT_DIR=$(create_snapshot_dir hot "$TIMESTAMP")
REMOTE_HOT=${VVTV_BACKUP_REMOTE_HOT:-}
REMOTE_FLAGS=${VVTV_BACKUP_REMOTE_FLAGS:---fast-list}

log INFO "Iniciando backup HOT"
log INFO "Snapshot: $SNAPSHOT_DIR"

# Copiar HLS em tempo real
rsync_copy "$VVTV_BROADCAST_DIR/hls/" "$SNAPSHOT_DIR/hls" "HLS"

# Copiar fila pronta (últimas 6h)
if [ -d "$VVTV_STORAGE_DIR/ready" ]; then
    mkdir -p "$SNAPSHOT_DIR/ready"
    find "$VVTV_STORAGE_DIR/ready" -maxdepth 1 -mindepth 1 -type d -mmin -360 -print0 |
        while IFS= read -r -d '' dir; do
            name=$(basename "$dir")
            rsync_copy "$dir/" "$SNAPSHOT_DIR/ready/$name" "ready/$name"
        done
fi

# Persistir bases de dados críticas
mkdir -p "$SNAPSHOT_DIR/data"
for db in plans queue metrics economy viewers; do
    file="$VVTV_DATA_DIR/${db}.sqlite"
    if [ -f "$file" ]; then
        log INFO "Copiando $db.sqlite"
        cp "$file" "$SNAPSHOT_DIR/data/${db}.sqlite"
    else
        log WARN "Banco $file não encontrado"
    fi
done

# Sincronizar com destino remoto imediato
if rclone_sync "$SNAPSHOT_DIR" "$REMOTE_HOT" "$REMOTE_FLAGS"; then
    rclone_check "$SNAPSHOT_DIR" "$REMOTE_HOT" "$REMOTE_FLAGS" || true
fi

# Limpeza de snapshots antigos (24h)
rotate_backups "$VVTV_BACKUP_DIR/hot" $((24 * 3600))

log INFO "Backup HOT concluído"
