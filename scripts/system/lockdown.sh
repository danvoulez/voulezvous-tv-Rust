#!/usr/bin/env bash
# VVTV M2 - System Lockdown for Hibernation
# Makes critical directories read-only and starts guardian daemon

set -euo pipefail

BASE="/vvtv"
VAULT="$BASE/vault"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[LOCKDOWN]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[LOCKDOWN WARN]${NC} $1"
}

error() {
    echo -e "${RED}[LOCKDOWN ERROR]${NC} $1"
    exit 1
}

# Check if running as root (needed for some operations)
if [[ $EUID -eq 0 ]]; then
    SUDO=""
else
    SUDO="sudo"
fi

log "Starting system lockdown for hibernation..."

# Step 1: Stop all VVTV services
log "Stopping VVTV services..."

# Stop systemd services (Linux)
if command -v systemctl &> /dev/null; then
    for service in vvtv-planner vvtv-realizer vvtv-discovery vvtv-watchdog nginx-rtmp; do
        $SUDO systemctl stop "$service" 2>/dev/null || warn "Could not stop $service"
        $SUDO systemctl disable "$service" 2>/dev/null || warn "Could not disable $service"
    done
fi

# Stop launchd services (macOS)
if command -v launchctl &> /dev/null; then
    for plist in /Library/LaunchDaemons/vvtv.*.plist; do
        if [[ -f "$plist" ]]; then
            $SUDO launchctl unload -w "$plist" 2>/dev/null || warn "Could not unload $plist"
        fi
    done
fi

# Step 2: Make critical directories read-only
log "Setting directories to read-only..."

# Set immutable flags on snapshots and manifests
if [[ -d "$VAULT/snapshots" ]]; then
    # Linux: use chattr +i
    if command -v chattr &> /dev/null; then
        find "$VAULT/snapshots" -type f -exec $SUDO chattr +i {} \; 2>/dev/null || warn "Could not set immutable flags (chattr)"
    fi
    
    # macOS: use chflags uchg
    if command -v chflags &> /dev/null; then
        find "$VAULT/snapshots" -type f -exec $SUDO chflags uchg {} \; 2>/dev/null || warn "Could not set immutable flags (chflags)"
    fi
    
    # Fallback: change permissions
    find "$VAULT/snapshots" -type f -exec chmod 444 {} \; 2>/dev/null || warn "Could not change permissions"
    find "$VAULT/snapshots" -type d -exec chmod 555 {} \; 2>/dev/null || warn "Could not change directory permissions"
fi

# Make key directories read-only
for dir in "$BASE/system/configs" "$BASE/data" "$VAULT/keys"; do
    if [[ -d "$dir" ]]; then
        find "$dir" -type f -exec chmod 444 {} \; 2>/dev/null || warn "Could not make $dir read-only"
        find "$dir" -type d -exec chmod 555 {} \; 2>/dev/null || warn "Could not make $dir directories read-only"
    fi
done

# Step 3: Create hibernation state file
HIBERNATION_STATE="$VAULT/hibernation_state.jsonl"
cat >> "$HIBERNATION_STATE" << EOF
{"state":"hibernating","timestamp":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","lockdown_by":"$(whoami)","hostname":"$(hostname)"}
EOF

log "Hibernation state recorded in $HIBERNATION_STATE"

# Step 4: Start sleep guardian daemon (if available)
SLEEPGUARD_SCRIPT="$BASE/scripts/system/sleepguardd.sh"
if [[ -f "$SLEEPGUARD_SCRIPT" ]]; then
    log "Starting sleep guardian daemon..."
    nohup "$SLEEPGUARD_SCRIPT" > "$VAULT/sleepguard.log" 2>&1 &
    echo $! > "$VAULT/sleepguard.pid"
    log "Sleep guardian started with PID $(cat "$VAULT/sleepguard.pid")"
else
    warn "Sleep guardian script not found at $SLEEPGUARD_SCRIPT"
fi

# Step 5: Create resume script for easy recovery
cat > "$BASE/scripts/system/resume.sh" << 'EOF'
#!/usr/bin/env bash
# VVTV M2 - Resume from Hibernation

set -euo pipefail

BASE="/vvtv"
VAULT="$BASE/vault"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[RESUME]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[RESUME WARN]${NC} $1"
}

# Check if running as root for some operations
if [[ $EUID -eq 0 ]]; then
    SUDO=""
else
    SUDO="sudo"
fi

log "Resuming VVTV from hibernation..."

# Stop sleep guardian if running
if [[ -f "$VAULT/sleepguard.pid" ]]; then
    GUARD_PID=$(cat "$VAULT/sleepguard.pid")
    if kill -0 "$GUARD_PID" 2>/dev/null; then
        kill "$GUARD_PID"
        log "Stopped sleep guardian (PID $GUARD_PID)"
    fi
    rm -f "$VAULT/sleepguard.pid"
fi

# Remove immutable flags
log "Removing read-only protections..."

if [[ -d "$VAULT/snapshots" ]]; then
    # Linux: remove chattr +i
    if command -v chattr &> /dev/null; then
        find "$VAULT/snapshots" -type f -exec $SUDO chattr -i {} \; 2>/dev/null || warn "Could not remove immutable flags (chattr)"
    fi
    
    # macOS: remove chflags uchg
    if command -v chflags &> /dev/null; then
        find "$VAULT/snapshots" -type f -exec $SUDO chflags nouchg {} \; 2>/dev/null || warn "Could not remove immutable flags (chflags)"
    fi
fi

# Restore write permissions
for dir in "$BASE/system/configs" "$BASE/data" "$VAULT/keys"; do
    if [[ -d "$dir" ]]; then
        find "$dir" -type f -exec chmod 644 {} \; 2>/dev/null || warn "Could not restore permissions for $dir"
        find "$dir" -type d -exec chmod 755 {} \; 2>/dev/null || warn "Could not restore directory permissions for $dir"
    fi
done

# Restart services in order
log "Restarting services..."

# Start systemd services (Linux)
if command -v systemctl &> /dev/null; then
    for service in nginx-rtmp vvtv-planner vvtv-realizer vvtv-discovery vvtv-watchdog; do
        $SUDO systemctl enable "$service" 2>/dev/null || warn "Could not enable $service"
        $SUDO systemctl start "$service" 2>/dev/null || warn "Could not start $service"
    done
fi

# Start launchd services (macOS)
if command -v launchctl &> /dev/null; then
    for plist in /Library/LaunchDaemons/vvtv.*.plist; do
        if [[ -f "$plist" ]]; then
            $SUDO launchctl load -w "$plist" 2>/dev/null || warn "Could not load $plist"
        fi
    done
fi

# Resume VVTV operations
if command -v vvtvctl &> /dev/null; then
    log "Resuming VVTV operations..."
    vvtvctl plan resume 2>/dev/null || warn "Could not resume planner"
    vvtvctl queue freeze --off 2>/dev/null || warn "Could not unfreeze queue"
    
    # Wait a moment for services to stabilize
    sleep 5
    
    # Start broadcaster
    vvtvctl broadcaster start 2>/dev/null || warn "Could not start broadcaster"
fi

# Record resume in state file
cat >> "$VAULT/hibernation_state.jsonl" << EOF
{"state":"resumed","timestamp":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","resumed_by":"$(whoami)","hostname":"$(hostname)"}
EOF

log "System resumed from hibernation successfully!"
log "Run 'vvtvctl health check' to verify system status"
EOF

chmod +x "$BASE/scripts/system/resume.sh"

log "System lockdown completed successfully!"
echo
echo "🔒 System is now in hibernation mode:"
echo "   - All services stopped"
echo "   - Critical directories are read-only"
echo "   - Sleep guardian is monitoring integrity"
echo
echo "📋 To resume operations:"
echo "   ./scripts/system/resume.sh"
echo
echo "📊 Monitor hibernation:"
echo "   tail -f $VAULT/sleepguard.log"