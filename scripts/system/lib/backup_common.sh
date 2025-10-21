#!/bin/bash
# Common helpers for VVTV backup scripts

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
VVTV_LOG_DIR=${VVTV_LOG_DIR:-$VVTV_BASE_DIR/system/logs}
VVTV_BACKUP_DIR=${VVTV_BACKUP_DIR:-$VVTV_BASE_DIR/vault/snapshots}
VVTV_DATA_DIR=${VVTV_DATA_DIR:-$VVTV_BASE_DIR/data}
VVTV_STORAGE_DIR=${VVTV_STORAGE_DIR:-$VVTV_BASE_DIR/storage}
VVTV_BROADCAST_DIR=${VVTV_BROADCAST_DIR:-$VVTV_BASE_DIR/broadcast}

mkdir -p "$VVTV_LOG_DIR" "$VVTV_BACKUP_DIR"

log() {
    local level=$1
    shift
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [$level] $*" | tee -a "$LOG_FILE"
}

require_command() {
    local name=$1
    if ! command -v "$name" >/dev/null 2>&1; then
        log WARN "Dependência '$name' não encontrada. Continuando em modo degradado."
        return 1
    fi
    return 0
}

rsync_copy() {
    local source=$1
    local destination=$2
    local label=${3:-$destination}

    if require_command rsync; then
        log INFO "Iniciando sincronização rsync para $label"
        rsync -a --delete "$source" "$destination"
    else
        log WARN "rsync indisponível, usando cp -a (sem --delete)"
        mkdir -p "$destination"
        cp -a "$source" "$destination"
    fi
}

rclone_sync() {
    local source=$1
    local remote=$2
    local extra_flags=$3
    if [[ -z "$remote" ]]; then
        return 0
    fi
    if ! require_command rclone; then
        log WARN "rclone não disponível, pulando sync remoto para $remote"
        return 0
    fi
    if [[ -f "$source" ]]; then
        local basename
        basename=$(basename "$source")
        local destination="$remote"
        if [[ "$destination" != *"$basename" ]]; then
            destination="${destination%/}/$basename"
        fi
        log INFO "Enviando arquivo $basename → $destination"
        if ! rclone copyto $extra_flags "$source" "$destination" >>"$LOG_FILE" 2>&1; then
            log ERROR "rclone copyto falhou para $destination"
            return 1
        fi
    else
        log INFO "Sincronizando diretório $source → $remote"
        if ! rclone sync $extra_flags "$source" "$remote" >>"$LOG_FILE" 2>&1; then
            log ERROR "rclone sync falhou para $remote"
            return 1
        fi
    fi
    return 0
}

rclone_check() {
    local source=$1
    local remote=$2
    local extra_flags=$3
    if [[ -z "$remote" ]]; then
        return 0
    fi
    if ! command -v rclone >/dev/null 2>&1; then
        return 0
    fi
    log INFO "Verificando consistência $source ↔ $remote"
    if ! rclone check $extra_flags "$source" "$remote" >>"$LOG_FILE" 2>&1; then
        log ERROR "rclone check encontrou divergências para $remote"
        return 1
    fi
    return 0
}

rotate_backups() {
    local dir=$1
    local retention_seconds=$2
    mkdir -p "$dir"
    find "$dir" -mindepth 1 -maxdepth 1 \( -type d -o -type f \) \
        -mmin +$((retention_seconds / 60)) -print0 |
        while IFS= read -r -d '' item; do
            log INFO "Removendo snapshot expirado $(basename "$item")"
            rm -rf "$item"
        done
}

create_snapshot_dir() {
    local tier=$1
    local timestamp=$2
    local target="$VVTV_BACKUP_DIR/$tier/$timestamp"
    mkdir -p "$target"
    echo "$target"
}

compress_tar() {
    local source=$1
    local destination=$2
    local label=$3
    if require_command tar && require_command zstd; then
        log INFO "Compactando $label para $(basename "$destination")"
        tar -C "$source" -cf - . | zstd -T0 -19 -o "$destination"
    else
        log WARN "tar ou zstd indisponível, copiando sem compressão"
        rsync_copy "$source/" "$destination" "$label"
    fi
}

