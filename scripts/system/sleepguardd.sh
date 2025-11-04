#!/usr/bin/env bash
# VVTV Sleep Guardian Daemon
# Monitors system integrity during hibernation

set -euo pipefail

BASE="/vvtv"
VAULT="$BASE/vault"
REPORT_FILE="$VAULT/sleepguard_report.jsonl"
CHECK_INTERVAL=900  # 15 minutes

# Ensure report file exists
touch "$REPORT_FILE"

log_report() {
    local status="$1"
    local message="$2"
    local details="${3:-}"
    
    cat >> "$REPORT_FILE" << EOF
{"timestamp":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","status":"$status","message":"$message","details":"$details","hostname":"$(hostname)"}
EOF
}

check_integrity() {
    local errors=0
    local warnings=0
    
    # Check if critical directories still exist
    for dir in "$BASE/data" "$BASE/system" "$VAULT/snapshots"; do
        if [[ ! -d "$dir" ]]; then
            log_report "ERROR" "Critical directory missing" "$dir"
            ((errors++))
        fi
    done
    
    # Sample file integrity check (check 10% of files each run)
    if [[ -d "$VAULT/snapshots" ]]; then
        local snapshot_files=($(find "$VAULT/snapshots" -type f -name "*.tar.zst" -o -name "*.json" -o -name "*.sig" 2>/dev/null))
        local sample_size=$((${#snapshot_files[@]} / 10 + 1))
        
        for ((i=0; i<sample_size && i<${#snapshot_files[@]}; i++)); do
            local file="${snapshot_files[$i]}"
            if [[ -f "$file" ]]; then
                # Check if file is readable and hasn't been modified recently
                local mtime=$(stat -c %Y "$file" 2>/dev/null || echo "0")
                local now=$(date +%s)
                local age=$((now - mtime))
                
                # If file was modified in the last hour, that's suspicious
                if [[ $age -lt 3600 ]]; then
                    log_report "WARNING" "Snapshot file recently modified" "$file (age: ${age}s)"
                    ((warnings++))
                fi
                
                # Check if file is still readable
                if ! head -c 1 "$file" >/dev/null 2>&1; then
                    log_report "ERROR" "Snapshot file not readable" "$file"
                    ((errors++))
                fi
            fi
        done
    fi
    
    # Check disk space
    local disk_usage=$(df "$BASE" | awk 'NR==2 {print $5}' | sed 's/%//' || echo "100")
    if [[ $disk_usage -gt 90 ]]; then
        log_report "ERROR" "Disk space critical" "${disk_usage}% used"
        ((errors++))
    elif [[ $disk_usage -gt 80 ]]; then
        log_report "WARNING" "Disk space high" "${disk_usage}% used"
        ((warnings++))
    fi
    
    # Check for unexpected processes
    local vvtv_processes=$(ps aux | grep -E "(vvtv|ffmpeg|chromium)" | grep -v grep | wc -l || echo "0")
    if [[ $vvtv_processes -gt 0 ]]; then
        log_report "WARNING" "Unexpected VVTV processes running" "$vvtv_processes processes"
        ((warnings++))
    fi
    
    # Check system load
    local load_avg=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $1}' | sed 's/,//' || echo "0")
    if (( $(echo "$load_avg > 2.0" | bc -l 2>/dev/null || echo "0") )); then
        log_report "WARNING" "High system load during hibernation" "load: $load_avg"
        ((warnings++))
    fi
    
    # Report status
    if [[ $errors -eq 0 && $warnings -eq 0 ]]; then
        log_report "OK" "System integrity check passed" "checked $sample_size files"
    else
        log_report "ISSUES" "Integrity check completed with issues" "errors: $errors, warnings: $warnings"
    fi
}

# Main daemon loop
log_report "INFO" "Sleep guardian daemon started" "PID: $$, interval: ${CHECK_INTERVAL}s"

while true; do
    check_integrity
    sleep $CHECK_INTERVAL
done