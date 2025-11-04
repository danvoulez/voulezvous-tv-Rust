#!/bin/bash
# VVTV security audit wrapper

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
REPORT_DIR=${VVTV_SECURITY_REPORT_DIR:-$VVTV_BASE_DIR/security}
LOG_FILE=${VVTV_SECURITY_LOG:-$VVTV_BASE_DIR/system/logs/security.log}

mkdir -p "$REPORT_DIR" "$(dirname "$LOG_FILE")"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [audit] $*" | tee -a "$LOG_FILE"
}

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
REPORT="$REPORT_DIR/audit_$STAMP.txt"

if command -v lynis >/dev/null 2>&1; then
    log "Executando lynis audit system"
    lynis audit system --quiet --auditor "VVTV" >"$REPORT" 2>&1 || log "Audit retornou código $?"
    log "Relatório salvo em $REPORT"
else
    log "lynis não encontrado. Registrando checklist manual"
    cat >"$REPORT" <<MANUAL
# VVTV Security Audit Placeholder
Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)
Status: lynis não instalado neste host.
Ações sugeridas:
- sudo apt install lynis
- Executar: sudo lynis audit system --quiet --auditor "VVTV"
MANUAL
fi

