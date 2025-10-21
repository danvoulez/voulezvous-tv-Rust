#!/bin/bash
# VVTV Daily Self-Check with auto remediation

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
VVTV_DATA_DIR=${VVTV_DATA_DIR:-$VVTV_BASE_DIR/data}
VVTV_CACHE_DIR=${VVTV_CACHE_DIR:-$VVTV_BASE_DIR/cache}
VVTV_LOG_DIR=${VVTV_LOG_DIR:-$VVTV_BASE_DIR/system/logs}
VVTV_REPORT_DIR=${VVTV_REPORT_DIR:-$VVTV_BASE_DIR/system/reports}
VVTV_SCRIPTS_DIR=${VVTV_SCRIPTS_DIR:-$VVTV_BASE_DIR/system/bin}
VVTV_BROADCAST_DIR=${VVTV_BROADCAST_DIR:-$VVTV_BASE_DIR/broadcast}

mkdir -p "$VVTV_LOG_DIR" "$VVTV_REPORT_DIR"

REPORT="$VVTV_REPORT_DIR/selfcheck_$(date -u +%Y%m%dT%H%M%SZ).json"
LOG="$VVTV_LOG_DIR/selfcheck.log"

CHECKS_PASSED=0
CHECKS_FAILED=0
ISSUES=()
ACTIONS=()

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG"
}

run_hook() {
    local script=$1
    shift
    if [[ -x "$VVTV_SCRIPTS_DIR/$script" ]]; then
        "$VVTV_SCRIPTS_DIR/$script" "$@" >>"$LOG" 2>&1 || true
    fi
}

cleanup_cache() {
    if [[ -d "$VVTV_CACHE_DIR" ]]; then
        find "$VVTV_CACHE_DIR" -mindepth 1 -maxdepth 3 -type f -mmin +60 -delete 2>/dev/null || true
    fi
}

json_array() {
    if [[ $# -eq 0 ]]; then
        echo "[]"
        return
    fi
    if command -v jq >/dev/null 2>&1; then
        printf '%s\n' "$@" | jq -R . | jq -s .
    else
        local first=1
        printf '['
        for item in "$@"; do
            local escaped=${item//\\/\\\\}
            escaped=${escaped//"/\\"}
            if [[ $first -eq 0 ]]; then
                printf ', '
            fi
            printf '"%s"' "$escaped"
            first=0
        done
        printf ']'
    fi
}

restart_service() {
    local name=$1
    if command -v systemctl >/dev/null 2>&1; then
        systemctl restart "$name" >>"$LOG" 2>&1 || true
    else
        run_hook "restart_${name}.sh"
    fi
}

reapply_time_sync() {
    if command -v chronyc >/dev/null 2>&1; then
        chronyc makestep >>"$LOG" 2>&1 || true
    elif command -v ntpdate >/dev/null 2>&1; then
        ntpdate -u pool.ntp.org >>"$LOG" 2>&1 || true
    elif command -v sntp >/dev/null 2>&1; then
        sntp -s pool.ntp.org >>"$LOG" 2>&1 || true
    fi
}

check() {
    local name="$1"
    local command="$2"
    local remediation="$3"

    if eval "$command" >>"$LOG" 2>&1; then
        log "✅ $name"
        ((CHECKS_PASSED++))
        return 0
    else
        log "❌ $name"
        ISSUES+=("$name")
        ((CHECKS_FAILED++))
        if [[ -n "$remediation" ]]; then
            log "↻ Tentando correção: $remediation"
            eval "$remediation" >>"$LOG" 2>&1 || true
            ACTIONS+=("$name: $remediation")
        fi
        return 1
    fi
}

# Database integrity
check "plans.sqlite integrity" \
    "sqlite3 $VVTV_DATA_DIR/plans.sqlite 'PRAGMA integrity_check;' | grep -q '^ok$'" \
    "sqlite3 $VVTV_DATA_DIR/plans.sqlite 'PRAGMA wal_checkpoint(TRUNCATE);'"

check "queue.sqlite integrity" \
    "sqlite3 $VVTV_DATA_DIR/queue.sqlite 'PRAGMA integrity_check;' | grep -q '^ok$'" \
    "sqlite3 $VVTV_DATA_DIR/queue.sqlite 'PRAGMA wal_checkpoint(TRUNCATE);'"

# Disk usage
DISK_USAGE=$(df "$VVTV_BASE_DIR" | tail -1 | awk '{print $5}' | tr -d '%')
if [[ -z "$DISK_USAGE" ]]; then
    DISK_USAGE=0
fi
if (( DISK_USAGE < 80 )); then
    log "✅ Disk usage: ${DISK_USAGE}%"
    ((CHECKS_PASSED++))
else
    log "❌ Disk usage crítico: ${DISK_USAGE}%"
    ISSUES+=("Disk usage ${DISK_USAGE}%")
    ACTIONS+=("Disk cleanup executed")
    cleanup_cache
    ((CHECKS_FAILED++))
fi

# Temperature sensors (Linux/macOS fallback)
CPU_TEMP=""
if command -v osx-cpu-temp >/dev/null 2>&1; then
    CPU_TEMP=$(osx-cpu-temp -c | cut -d'°' -f1)
elif [[ -f /sys/class/thermal/thermal_zone0/temp ]]; then
    CPU_TEMP=$(awk '{print $1/1000}' /sys/class/thermal/thermal_zone0/temp)
fi
if [[ -n "$CPU_TEMP" ]]; then
    if (( $(echo "$CPU_TEMP < 75" | bc -l) )); then
        log "✅ CPU temp: ${CPU_TEMP}°C"
        ((CHECKS_PASSED++))
    else
        log "❌ CPU temp alta: ${CPU_TEMP}°C"
        ISSUES+=("CPU temp ${CPU_TEMP}°C")
        ACTIONS+=("Verificar ventilação")
        ((CHECKS_FAILED++))
    fi
fi

# Service checks
check "NGINX running" "pgrep -f nginx" "restart_service nginx"
check "Tailscale running" "tailscale status" "restart_service tailscaled"
check "FFmpeg encoder" "pgrep -f 'ffmpeg.*rtmp'" "run_hook restart_encoder.sh"

# Playlist availability
check "HLS playlist exists" "test -f $VVTV_BROADCAST_DIR/hls/live.m3u8" "run_hook fill_buffer.sh --dry-run"

# Clock sync
check "Clock synchronized" "timedatectl status 2>/dev/null | grep -qi 'synchronized: yes'" "reapply_time_sync"

# Generate JSON report
issues_json=$(json_array "${ISSUES[@]:-}")
actions_json=$(json_array "${ACTIONS[@]:-}")
cat >"$REPORT" <<JSON
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "checks_passed": $CHECKS_PASSED,
  "checks_failed": $CHECKS_FAILED,
  "disk_usage_percent": $DISK_USAGE,
  "cpu_temp_c": ${CPU_TEMP:-null},
  "issues": $issues_json,
  "actions": $actions_json
}
JSON

log "📊 Report saved: $REPORT"
log "Summary: $CHECKS_PASSED passed, $CHECKS_FAILED failed"

if (( CHECKS_FAILED > 0 )); then
    log "⚠️  Self-check completed with issues"
    if command -v telegram-send >/dev/null 2>&1; then
        telegram-send "⚠️ VVTV selfcheck falhou em $(hostname -s). Verificar $REPORT"
    fi
    exit 1
else
    log "✅ Self-check completed successfully"
    exit 0
fi
