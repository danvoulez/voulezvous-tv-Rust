#!/bin/bash
# VVTV Warm Backup (every 6 hours)

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$SCRIPT_DIR/lib/backup_common.sh"

LOG_FILE=${VVTV_BACKUP_LOG_WARM:-$VVTV_LOG_DIR/backup_warm.log}
mkdir -p "$(dirname "$LOG_FILE")"

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
SNAPSHOT_DIR=$(create_snapshot_dir warm "$TIMESTAMP")
REMOTE_WARM=${VVTV_BACKUP_REMOTE_WARM:-}
REMOTE_FLAGS=${VVTV_BACKUP_REMOTE_FLAGS:---fast-list}

log INFO "Iniciando backup WARM"
log INFO "Snapshot: $SNAPSHOT_DIR"

mkdir -p "$SNAPSHOT_DIR"

# Copiar diretórios críticos completos
rsync_copy "$VVTV_STORAGE_DIR/ready/" "$SNAPSHOT_DIR/ready" "ready"
rsync_copy "$VVTV_BROADCAST_DIR/hls/" "$SNAPSHOT_DIR/hls" "hls"

mkdir -p "$SNAPSHOT_DIR/data"
for db in plans queue metrics economy viewers; do
    file="$VVTV_DATA_DIR/${db}.sqlite"
    if [ -f "$file" ]; then
        cp "$file" "$SNAPSHOT_DIR/data/${db}.sqlite"
    fi
done

# Registrar metadados
if command -v sha256sum >/dev/null 2>&1; then
    (cd "$SNAPSHOT_DIR" && find . -type f -print0 | sort -z | xargs -0 sha256sum) >"$SNAPSHOT_DIR/checksums.sha256"
fi

# Compactar snapshot para arquivo único
ARCHIVE_PATH="$VVTV_BACKUP_DIR/warm/${TIMESTAMP}.tar.zst"
mkdir -p "$(dirname "$ARCHIVE_PATH")"
compress_tar "$SNAPSHOT_DIR" "$ARCHIVE_PATH" "warm-$TIMESTAMP"

# Sincronizar arquivo com armazenamento remoto
if rclone_sync "$ARCHIVE_PATH" "$REMOTE_WARM" "$REMOTE_FLAGS"; then
    rclone_check "$ARCHIVE_PATH" "$REMOTE_WARM" "$REMOTE_FLAGS" || true
fi

# Limpeza (manter diretório temporário até sucesso)
rm -rf "$SNAPSHOT_DIR"
rotate_backups "$VVTV_BACKUP_DIR/warm" $((72 * 3600))

log INFO "Backup WARM concluído"
