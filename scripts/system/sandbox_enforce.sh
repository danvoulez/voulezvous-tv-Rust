#!/bin/bash
# VVTV Sandbox hardening routine

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
LOG_FILE=${VVTV_SECURITY_LOG:-$VVTV_BASE_DIR/system/logs/security.log}
CRITICAL_SCRIPTS=(
    "$VVTV_BASE_DIR/system/bin/selfcheck.sh"
    "$VVTV_BASE_DIR/system/bin/backup_hot.sh"
    "$VVTV_BASE_DIR/system/bin/backup_warm.sh"
    "$VVTV_BASE_DIR/system/bin/backup_cold.sh"
    "$VVTV_BASE_DIR/system/bin/standby.sh"
    "$VVTV_BASE_DIR/system/bin/resume.sh"
)

mkdir -p "$(dirname "$LOG_FILE")"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [sandbox] $*" | tee -a "$LOG_FILE"
}

log "Aplicando chattr +i a scripts críticos"
if command -v chattr >/dev/null 2>&1; then
    for script in "${CRITICAL_SCRIPTS[@]}"; do
        if [[ -f "$script" ]]; then
            chattr +i "$script" 2>/dev/null || log "Aviso: falha ao proteger $script"
        fi
    done
else
    log "chattr indisponível, pulando imutabilidade"
fi

log "Configurando namespaces mínimos"
if command -v unshare >/dev/null 2>&1; then
    if [[ ! -f /etc/systemd/system/vvtv-sandbox.service ]]; then
        cat <<SERVICE | sudo tee /etc/systemd/system/vvtv-sandbox.service >/dev/null
[Unit]
Description=VVTV sandbox namespace

[Service]
Type=oneshot
ExecStart=/usr/bin/unshare --mount --uts --ipc --pid --fork --mount-proc /bin/true
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
SERVICE
        sudo systemctl enable vvtv-sandbox.service >/dev/null 2>&1 || true
    fi
else
    log "unshare indisponível, revisar manualmente"
fi

log "Aplicando limites cgroup básicos"
if command -v systemctl >/dev/null 2>&1; then
    sudo systemctl set-property --runtime vvtv.service CPUQuota=180% MemoryMax=16G >/dev/null 2>&1 || true
    sudo systemctl set-property --runtime vvtv-broadcaster.service MemoryHigh=8G >/dev/null 2>&1 || true
else
    log "systemctl indisponível, cgroups não aplicados"
fi

log "Sandbox enforcement concluído"
