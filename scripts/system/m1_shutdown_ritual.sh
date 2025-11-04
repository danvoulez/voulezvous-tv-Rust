#!/usr/bin/env bash
# VVTV M1 - Shutdown Ritual with Cryptographic Proof
# Creates verifiable snapshots and graceful system shutdown

set -euo pipefail

# Configuration
STAMP="$(date -u +'%Y-%m-%d_%H%MZ')"
BASE="/vvtv"
VAULT="$BASE/vault"
SNAP="$VAULT/snapshots/$STAMP"
SYSTEM_DIR="$BASE/system"
DATA_DIR="$BASE/data"
STORAGE_DIR="$BASE/storage"
BROADCAST_DIR="$BASE/broadcast"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[M1 $(date -u +%H:%M:%S)]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[M1 WARN]${NC} $1"
}

error() {
    echo -e "${RED}[M1 ERROR]${NC} $1"
    exit 1
}

# Create snapshot directory
mkdir -p "$SNAP"
log "Created snapshot directory: $SNAP"

# Step 1: Pre-checks (hard stop if failed)
log "Running pre-flight checks..."

# Check if vvtvctl is available
if ! command -v vvtvctl &> /dev/null; then
    error "vvtvctl command not found"
fi

# Check queue buffer
BUFFER_MINUTES=$(vvtvctl queue summary --format json 2>/dev/null | jq -r '.buffer_duration_minutes // 0' || echo "0")
if (( $(echo "$BUFFER_MINUTES < 60" | bc -l) )); then
    warn "Buffer is only ${BUFFER_MINUTES} minutes (< 60min recommended)"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        error "Aborted by user due to low buffer"
    fi
fi

# Check database integrity
log "Checking database integrity..."
for db in "$DATA_DIR"/*.sqlite; do
    if [[ -f "$db" ]]; then
        log "Checking $(basename "$db")..."
        if ! sqlite3 "$db" "PRAGMA integrity_check;" | grep -q "ok"; then
            error "Database integrity check failed for $db"
        fi
    fi
done

# Check disk space (need at least 15% free)
DISK_FREE=$(df "$BASE" | awk 'NR==2 {print $5}' | sed 's/%//')
if (( DISK_FREE > 85 )); then
    error "Disk usage is ${DISK_FREE}% (need <85% for safe operation)"
fi

log "Pre-flight checks passed ✓"

# Step 2: Freeze schedulers
log "Freezing system schedulers..."

# Pause planner, realizer, discovery
vvtvctl plan pause 2>/dev/null || warn "Could not pause planner"
vvtvctl queue freeze --on 2>/dev/null || warn "Could not freeze queue"

# Wait for in-progress operations to complete
log "Waiting for in-progress operations to complete..."
TIMEOUT=600  # 10 minutes
ELAPSED=0
while [[ $ELAPSED -lt $TIMEOUT ]]; do
    # Check if any critical processes are still running
    IN_PROGRESS=$(ps aux | grep -E "(ffmpeg|chromium)" | grep -v grep | wc -l || echo "0")
    if [[ "$IN_PROGRESS" -eq "0" ]]; then
        break
    fi
    log "Waiting for $IN_PROGRESS processes to complete... (${ELAPSED}s/${TIMEOUT}s)"
    sleep 10
    ELAPSED=$((ELAPSED + 10))
done

if [[ $ELAPSED -ge $TIMEOUT ]]; then
    warn "Timeout waiting for processes to complete, proceeding anyway"
fi

# Step 3: Capture final frame and telemetry
log "Capturing final frame and telemetry..."

# Try to capture final frame from HLS stream
if curl -s --max-time 5 "http://127.0.0.1:8080/hls/main.m3u8" > /dev/null 2>&1; then
    if command -v ffmpeg &> /dev/null; then
        timeout 30 ffmpeg -y -i "http://127.0.0.1:8080/hls/main.m3u8" \
            -frames:v 1 -q:v 2 "$SNAP/final_frame.jpg" 2>/dev/null || \
            warn "Could not capture final frame"
    else
        warn "ffmpeg not available, skipping final frame capture"
    fi
else
    warn "HLS stream not accessible, skipping final frame capture"
fi

# Capture telemetry
cat > "$SNAP/final_frame.json" << EOF
{
    "timestamp_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "shutdown_reason": "manual_ritual",
    "buffer_minutes": $BUFFER_MINUTES,
    "disk_free_percent": $((100 - DISK_FREE)),
    "system_uptime": "$(uptime -p 2>/dev/null || echo 'unknown')",
    "vvtv_version": "$(git -C "$BASE" rev-parse HEAD 2>/dev/null || echo 'unknown')",
    "capture_method": "m1_shutdown_ritual"
}
EOF

log "Telemetry captured ✓"

# Step 4: Create manifest with hashes
log "Calculating file hashes for manifest..."

create_manifest() {
    cat > "$SNAP/last_manifest.json" << EOF
{
    "timestamp_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "snapshot_id": "$STAMP",
    "vvtv_version": "$(git -C "$BASE" rev-parse HEAD 2>/dev/null || echo 'unknown')",
    "system_info": {
        "hostname": "$(hostname)",
        "os": "$(uname -s)",
        "arch": "$(uname -m)",
        "kernel": "$(uname -r)"
    },
    "directories": {
        "data": "$(find "$DATA_DIR" -type f -name "*.sqlite" -exec sha256sum {} \; 2>/dev/null | sort || echo '')",
        "system": "$(find "$SYSTEM_DIR" -type f -name "*.toml" -o -name "*.yaml" -exec sha256sum {} \; 2>/dev/null | sort || echo '')",
        "storage_count": "$(find "$STORAGE_DIR" -type f -name "*.mp4" 2>/dev/null | wc -l || echo '0')",
        "broadcast_segments": "$(find "$BROADCAST_DIR" -type f -name "*.ts" 2>/dev/null | wc -l || echo '0')"
    },
    "business_logic": {
        "config_hash": "$(sha256sum "$SYSTEM_DIR/business_logic.yaml" 2>/dev/null | cut -d' ' -f1 || echo 'missing')",
        "last_modified": "$(stat -c %Y "$SYSTEM_DIR/business_logic.yaml" 2>/dev/null || echo '0')"
    }
}
EOF
}

create_manifest
log "Manifest created ✓"

# Step 5: Create snapshots
log "Creating compressed snapshots..."

# Database snapshot (includes WAL files)
if [[ -d "$DATA_DIR" ]]; then
    log "Creating database snapshot..."
    tar -I 'zstd -19' -cf "$SNAP/snapshot_db.tar.zst" -C "$DATA_DIR" . 2>/dev/null || \
        warn "Database snapshot may be incomplete"
fi

# System configuration snapshot
if [[ -d "$SYSTEM_DIR" ]]; then
    log "Creating system snapshot..."
    tar -I 'zstd -19' -cf "$SNAP/snapshot_system.tar.zst" -C "$SYSTEM_DIR" . 2>/dev/null || \
        warn "System snapshot may be incomplete"
fi

# Data snapshot (storage and broadcast, excluding cache/tmp)
log "Creating data snapshot (this may take a while)..."
tar -I 'zstd -19' -cf "$SNAP/snapshot_data.tar.zst" \
    --exclude="*/cache/*" --exclude="*/tmp/*" --exclude="*/temp/*" \
    -C "$BASE" storage broadcast 2>/dev/null || \
    warn "Data snapshot may be incomplete"

# Step 6: Create signature (if signing key exists)
SIGNING_KEY="$VAULT/keys/foundation_ed25519"
if [[ -f "$SIGNING_KEY" ]]; then
    log "Signing manifest..."
    # Simple signature using openssl (Ed25519 if available, otherwise RSA)
    if openssl version | grep -q "OpenSSL 1.1" || openssl version | grep -q "OpenSSL 3"; then
        openssl dgst -sha256 -sign "$SIGNING_KEY" -out "$SNAP/signature.sig" "$SNAP/last_manifest.json" 2>/dev/null || \
            warn "Could not create signature"
    else
        warn "OpenSSL version does not support Ed25519, skipping signature"
    fi
else
    warn "Signing key not found at $SIGNING_KEY, skipping signature"
fi

# Step 7: Stop services gracefully
log "Stopping broadcast services..."

# Stop broadcaster if running
vvtvctl broadcaster stop 2>/dev/null || warn "Could not stop broadcaster"

# Stop nginx-rtmp if running
if command -v systemctl &> /dev/null; then
    sudo systemctl stop nginx-rtmp 2>/dev/null || warn "Could not stop nginx-rtmp via systemctl"
elif command -v launchctl &> /dev/null; then
    sudo launchctl unload /Library/LaunchDaemons/nginx-rtmp.plist 2>/dev/null || warn "Could not stop nginx-rtmp via launchctl"
fi

# Create shutdown marker
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$SNAP/shutdown_complete.marker"

# Final summary
log "Shutdown ritual completed successfully!"
echo
echo "📦 Snapshot Location: $SNAP"
echo "📋 Files created:"
echo "   - last_manifest.json (system state)"
echo "   - final_frame.json (telemetry)"
echo "   - snapshot_db.tar.zst (databases)"
echo "   - snapshot_system.tar.zst (configurations)"
echo "   - snapshot_data.tar.zst (media files)"
if [[ -f "$SNAP/signature.sig" ]]; then
    echo "   - signature.sig (cryptographic proof)"
fi
if [[ -f "$SNAP/final_frame.jpg" ]]; then
    echo "   - final_frame.jpg (last broadcast frame)"
fi
echo
echo "🔒 System is now safely shut down and ready for hibernation or resurrection."
echo "💾 Snapshot ID: $STAMP"

# Create resurrection instructions
cat > "$SNAP/resurrection_instructions.md" << 'EOF'
# VVTV Resurrection Instructions

## Quick Revival (Same Machine)
```bash
# 1. Resume services
./scripts/system/resume.sh

# 2. Verify system health
vvtvctl health check

# 3. Restart streaming
vvtvctl broadcaster start
```

## Full Resurrection (New Machine)
```bash
# 1. Install VVTV dependencies
# 2. Extract snapshots to /vvtv/
# 3. Run resurrection script
./scripts/system/m3_resurrection.sh --snapshot-dir /path/to/snapshot
```

## Verification
- Check database integrity: `vvtvctl health check`
- Verify configuration: `vvtvctl business-logic validate`
- Test streaming: `vvtvctl broadcaster status`
EOF

log "Resurrection instructions created at $SNAP/resurrection_instructions.md"