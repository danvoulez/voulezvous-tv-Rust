#!/usr/bin/env bash
# VVTV Comprehensive Health Check Script
# Validates all system components and generates health report

set -euo pipefail

BASE="/vvtv"
REPORT_FILE="/tmp/vvtv_health_$(date +%Y%m%d_%H%M%S).json"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[HEALTH]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[HEALTH WARN]${NC} $1"
}

error() {
    echo -e "${RED}[HEALTH ERROR]${NC} $1"
}

info() {
    echo -e "${BLUE}[HEALTH INFO]${NC} $1"
}

# Initialize report structure
init_report() {
    cat > "$REPORT_FILE" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "hostname": "$(hostname)",
    "overall_status": "unknown",
    "checks": {},
    "metrics": {},
    "recommendations": []
}
EOF
}

# Update report with check result
update_report() {
    local check_name="$1"
    local status="$2"
    local message="$3"
    local details="${4:-{}}"
    
    jq --arg name "$check_name" \
       --arg status "$status" \
       --arg message "$message" \
       --argjson details "$details" \
       '.checks[$name] = {
           "status": $status,
           "message": $message,
           "details": $details,
           "timestamp": (now | strftime("%Y-%m-%dT%H:%M:%SZ"))
       }' "$REPORT_FILE" > "$REPORT_FILE.tmp" && mv "$REPORT_FILE.tmp" "$REPORT_FILE"
}

# Add metric to report
add_metric() {
    local metric_name="$1"
    local value="$2"
    local unit="${3:-}"
    
    jq --arg name "$metric_name" \
       --argjson value "$value" \
       --arg unit "$unit" \
       '.metrics[$name] = {
           "value": $value,
           "unit": $unit,
           "timestamp": (now | strftime("%Y-%m-%dT%H:%M:%SZ"))
       }' "$REPORT_FILE" > "$REPORT_FILE.tmp" && mv "$REPORT_FILE.tmp" "$REPORT_FILE"
}

# Add recommendation
add_recommendation() {
    local recommendation="$1"
    local priority="${2:-medium}"
    
    jq --arg rec "$recommendation" \
       --arg priority "$priority" \
       '.recommendations += [{
           "message": $rec,
           "priority": $priority,
           "timestamp": (now | strftime("%Y-%m-%dT%H:%M:%SZ"))
       }]' "$REPORT_FILE" > "$REPORT_FILE.tmp" && mv "$REPORT_FILE.tmp" "$REPORT_FILE"
}

# Check system resources
check_system_resources() {
    log "Checking system resources..."
    
    # CPU usage
    local cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | sed 's/%us,//' || echo "0")
    if [[ -z "$cpu_usage" ]]; then
        # macOS alternative
        cpu_usage=$(ps -A -o %cpu | awk '{s+=$1} END {print s}' || echo "0")
    fi
    
    add_metric "cpu_usage_percent" "$cpu_usage" "percent"
    
    if (( $(echo "$cpu_usage > 80" | bc -l 2>/dev/null || echo "0") )); then
        update_report "cpu_usage" "warning" "High CPU usage: ${cpu_usage}%"
        add_recommendation "Consider reducing system load or scaling resources" "high"
    else
        update_report "cpu_usage" "ok" "CPU usage normal: ${cpu_usage}%"
    fi
    
    # Memory usage
    if command -v free &> /dev/null; then
        local mem_info=$(free -m)
        local mem_total=$(echo "$mem_info" | awk 'NR==2{print $2}')
        local mem_used=$(echo "$mem_info" | awk 'NR==2{print $3}')
        local mem_percent=$(( mem_used * 100 / mem_total ))
    else
        # macOS alternative
        local mem_total=$(sysctl -n hw.memsize | awk '{print int($1/1024/1024)}')
        local mem_used=$(ps -caxm -orss= | awk '{sum+=$1} END {print int(sum/1024)}')
        local mem_percent=$(( mem_used * 100 / mem_total ))
    fi
    
    add_metric "memory_usage_percent" "$mem_percent" "percent"
    add_metric "memory_total_mb" "$mem_total" "MB"
    add_metric "memory_used_mb" "$mem_used" "MB"
    
    if [[ $mem_percent -gt 90 ]]; then
        update_report "memory_usage" "critical" "Critical memory usage: ${mem_percent}%"
        add_recommendation "Immediate memory cleanup required" "critical"
    elif [[ $mem_percent -gt 80 ]]; then
        update_report "memory_usage" "warning" "High memory usage: ${mem_percent}%"
        add_recommendation "Monitor memory usage closely" "medium"
    else
        update_report "memory_usage" "ok" "Memory usage normal: ${mem_percent}%"
    fi
    
    # Disk usage
    local disk_usage=$(df "$BASE" | awk 'NR==2 {print $5}' | sed 's/%//')
    add_metric "disk_usage_percent" "$disk_usage" "percent"
    
    if [[ $disk_usage -gt 95 ]]; then
        update_report "disk_usage" "critical" "Critical disk usage: ${disk_usage}%"
        add_recommendation "Immediate disk cleanup required" "critical"
    elif [[ $disk_usage -gt 85 ]]; then
        update_report "disk_usage" "warning" "High disk usage: ${disk_usage}%"
        add_recommendation "Plan disk cleanup or expansion" "high"
    else
        update_report "disk_usage" "ok" "Disk usage normal: ${disk_usage}%"
    fi
}

# Check VVTV services
check_vvtv_services() {
    log "Checking VVTV services..."
    
    # Check if vvtvctl is available
    if command -v vvtvctl &> /dev/null; then
        update_report "vvtvctl" "ok" "vvtvctl command available"
        
        # Check business logic
        if vvtvctl business-logic validate &> /dev/null; then
            update_report "business_logic" "ok" "Business logic configuration valid"
        else
            update_report "business_logic" "error" "Business logic validation failed"
            add_recommendation "Fix business logic configuration" "high"
        fi
        
        # Check queue status
        local queue_info=$(vvtvctl queue summary --format json 2>/dev/null || echo '{}')
        local buffer_minutes=$(echo "$queue_info" | jq -r '.buffer_duration_minutes // 0')
        
        add_metric "queue_buffer_minutes" "$buffer_minutes" "minutes"
        
        if (( $(echo "$buffer_minutes < 30" | bc -l) )); then
            update_report "queue_buffer" "critical" "Queue buffer critically low: ${buffer_minutes} minutes"
            add_recommendation "Increase content buffer immediately" "critical"
        elif (( $(echo "$buffer_minutes < 60" | bc -l) )); then
            update_report "queue_buffer" "warning" "Queue buffer low: ${buffer_minutes} minutes"
            add_recommendation "Monitor buffer levels closely" "medium"
        else
            update_report "queue_buffer" "ok" "Queue buffer adequate: ${buffer_minutes} minutes"
        fi
        
    else
        update_report "vvtvctl" "error" "vvtvctl command not found"
        add_recommendation "Install or fix vvtvctl binary" "critical"
    fi
}

# Check databases
check_databases() {
    log "Checking database integrity..."
    
    local db_errors=0
    
    for db in "$BASE/data"/*.sqlite; do
        if [[ -f "$db" ]]; then
            local db_name=$(basename "$db" .sqlite)
            
            if sqlite3 "$db" "PRAGMA integrity_check;" | grep -q "ok"; then
                update_report "db_${db_name}" "ok" "Database integrity OK"
            else
                update_report "db_${db_name}" "error" "Database integrity check failed"
                add_recommendation "Repair or restore database: $db_name" "high"
                ((db_errors++))
            fi
            
            # Check database size
            local db_size=$(stat -c%s "$db" 2>/dev/null || stat -f%z "$db" 2>/dev/null || echo "0")
            local db_size_mb=$((db_size / 1024 / 1024))
            add_metric "db_${db_name}_size_mb" "$db_size_mb" "MB"
        fi
    done
    
    if [[ $db_errors -eq 0 ]]; then
        update_report "databases_overall" "ok" "All databases healthy"
    else
        update_report "databases_overall" "error" "$db_errors database(s) have issues"
    fi
}

# Check network connectivity
check_network() {
    log "Checking network connectivity..."
    
    # Check internet connectivity
    if ping -c 3 8.8.8.8 &> /dev/null; then
        update_report "internet_connectivity" "ok" "Internet connectivity available"
    else
        update_report "internet_connectivity" "warning" "Internet connectivity issues"
        add_recommendation "Check network configuration" "medium"
    fi
    
    # Check local services
    local services_to_check=(
        "127.0.0.1:7070:LLM Pool API"
        "127.0.0.1:8080:HLS Stream"
        "127.0.0.1:1935:RTMP Ingest"
    )
    
    for service_info in "${services_to_check[@]}"; do
        IFS=':' read -r host port name <<< "$service_info"
        
        if timeout 5 bash -c "</dev/tcp/$host/$port" 2>/dev/null; then
            update_report "service_${port}" "ok" "$name responding on port $port"
        else
            update_report "service_${port}" "warning" "$name not responding on port $port"
            add_recommendation "Check $name service status" "medium"
        fi
    done
}

# Check file permissions and ownership
check_permissions() {
    log "Checking file permissions..."
    
    local permission_issues=0
    
    # Check critical directories
    local critical_dirs=(
        "$BASE/data"
        "$BASE/system"
        "$BASE/vault"
        "$BASE/storage"
    )
    
    for dir in "${critical_dirs[@]}"; do
        if [[ -d "$dir" ]]; then
            local owner=$(stat -c '%U' "$dir" 2>/dev/null || stat -f '%Su' "$dir" 2>/dev/null || echo "unknown")
            
            if [[ "$owner" == "vvtv" ]] || [[ "$owner" == "$(whoami)" ]]; then
                update_report "permissions_$(basename "$dir")" "ok" "Directory ownership correct: $dir"
            else
                update_report "permissions_$(basename "$dir")" "warning" "Directory ownership issue: $dir (owner: $owner)"
                add_recommendation "Fix ownership for $dir" "medium"
                ((permission_issues++))
            fi
        else
            update_report "permissions_$(basename "$dir")" "error" "Critical directory missing: $dir"
            add_recommendation "Create missing directory: $dir" "high"
            ((permission_issues++))
        fi
    done
    
    if [[ $permission_issues -eq 0 ]]; then
        update_report "permissions_overall" "ok" "File permissions correct"
    else
        update_report "permissions_overall" "warning" "$permission_issues permission issues found"
    fi
}

# Determine overall status
determine_overall_status() {
    local critical_count=$(jq '[.checks[] | select(.status == "critical")] | length' "$REPORT_FILE")
    local error_count=$(jq '[.checks[] | select(.status == "error")] | length' "$REPORT_FILE")
    local warning_count=$(jq '[.checks[] | select(.status == "warning")] | length' "$REPORT_FILE")
    
    local overall_status
    if [[ $critical_count -gt 0 ]]; then
        overall_status="critical"
    elif [[ $error_count -gt 0 ]]; then
        overall_status="error"
    elif [[ $warning_count -gt 0 ]]; then
        overall_status="warning"
    else
        overall_status="healthy"
    fi
    
    jq --arg status "$overall_status" \
       '.overall_status = $status' "$REPORT_FILE" > "$REPORT_FILE.tmp" && mv "$REPORT_FILE.tmp" "$REPORT_FILE"
    
    # Add summary metrics
    add_metric "checks_total" "$(jq '.checks | length' "$REPORT_FILE")" "count"
    add_metric "checks_critical" "$critical_count" "count"
    add_metric "checks_error" "$error_count" "count"
    add_metric "checks_warning" "$warning_count" "count"
    add_metric "recommendations_total" "$(jq '.recommendations | length' "$REPORT_FILE")" "count"
}

# Print summary
print_summary() {
    local overall_status=$(jq -r '.overall_status' "$REPORT_FILE")
    local critical_count=$(jq '.metrics.checks_critical.value' "$REPORT_FILE")
    local error_count=$(jq '.metrics.checks_error.value' "$REPORT_FILE")
    local warning_count=$(jq '.metrics.checks_warning.value' "$REPORT_FILE")
    local total_checks=$(jq '.metrics.checks_total.value' "$REPORT_FILE")
    
    echo
    case "$overall_status" in
        "healthy")
            log "✅ VVTV System Health: HEALTHY"
            ;;
        "warning")
            warn "⚠️  VVTV System Health: WARNING ($warning_count warnings)"
            ;;
        "error")
            error "❌ VVTV System Health: ERROR ($error_count errors, $warning_count warnings)"
            ;;
        "critical")
            error "🚨 VVTV System Health: CRITICAL ($critical_count critical, $error_count errors)"
            ;;
    esac
    
    echo
    info "Health Check Summary:"
    echo "  Total checks: $total_checks"
    echo "  Critical: $critical_count"
    echo "  Errors: $error_count"
    echo "  Warnings: $warning_count"
    echo "  Report: $REPORT_FILE"
    
    # Show top recommendations
    local rec_count=$(jq '.recommendations | length' "$REPORT_FILE")
    if [[ $rec_count -gt 0 ]]; then
        echo
        info "Top Recommendations:"
        jq -r '.recommendations | sort_by(.priority) | reverse | .[0:3] | .[] | "  • " + .message + " (" + .priority + ")"' "$REPORT_FILE"
    fi
}

# Main execution
main() {
    log "Starting VVTV health check..."
    
    init_report
    
    check_system_resources
    check_vvtv_services
    check_databases
    check_network
    check_permissions
    
    determine_overall_status
    print_summary
    
    # Exit with appropriate code
    local overall_status=$(jq -r '.overall_status' "$REPORT_FILE")
    case "$overall_status" in
        "healthy") exit 0 ;;
        "warning") exit 1 ;;
        "error") exit 2 ;;
        "critical") exit 3 ;;
    esac
}

# Handle command line arguments
case "${1:-}" in
    --json)
        main > /dev/null
        cat "$REPORT_FILE"
        ;;
    --quiet)
        main > /dev/null 2>&1
        ;;
    *)
        main
        ;;
esac