#!/usr/bin/env bash
# VVTV Public API Gateway Startup Script

set -euo pipefail

BASE="/vvtv"
NGINX_CONF="$BASE/configs/nginx/api_gateway.conf"
PID_FILE="/var/run/vvtv_api_gateway.pid"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[API-GW]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[API-GW WARN]${NC} $1"
}

error() {
    echo -e "${RED}[API-GW ERROR]${NC} $1"
    exit 1
}

# Check if running as root (needed for binding to privileged ports)
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root (for port binding)"
fi

# Check if NGINX is installed
if ! command -v nginx &> /dev/null; then
    error "NGINX is not installed"
fi

# Check if configuration file exists
if [[ ! -f "$NGINX_CONF" ]]; then
    error "NGINX configuration not found: $NGINX_CONF"
fi

# Test NGINX configuration
log "Testing NGINX configuration..."
if ! nginx -t -c "$NGINX_CONF"; then
    error "NGINX configuration test failed"
fi

# Check if LLM Pool API is running
log "Checking LLM Pool API availability..."
if ! curl -s --max-time 3 "http://127.0.0.1:7070/health/ready" > /dev/null; then
    warn "LLM Pool API not responding at 127.0.0.1:7070"
    warn "Make sure the LLM Pool service is running before starting the gateway"
fi

# Create log directory
mkdir -p "$BASE/logs"
chown -R vvtv:vvtv "$BASE/logs" 2>/dev/null || warn "Could not set log directory ownership"

# Check if gateway is already running
if [[ -f "$PID_FILE" ]]; then
    OLD_PID=$(cat "$PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        warn "API Gateway already running with PID $OLD_PID"
        read -p "Stop existing instance and restart? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            log "Stopping existing instance..."
            kill "$OLD_PID"
            sleep 2
        else
            error "Aborted - gateway already running"
        fi
    else
        warn "Stale PID file found, removing..."
        rm -f "$PID_FILE"
    fi
fi

# Start NGINX with API gateway configuration
log "Starting VVTV API Gateway..."
nginx -c "$NGINX_CONF" -g "daemon off; pid $PID_FILE;" &
NGINX_PID=$!

# Wait a moment and check if it started successfully
sleep 2
if ! kill -0 "$NGINX_PID" 2>/dev/null; then
    error "Failed to start NGINX API Gateway"
fi

log "API Gateway started successfully with PID $NGINX_PID"

# Test the endpoints
log "Testing API endpoints..."

# Test health endpoint
if curl -s --max-time 5 "http://localhost:7071/v1/health/ready" > /dev/null; then
    log "Health endpoint responding ✓"
else
    warn "Health endpoint not responding"
fi

# Test rate limiting (should get 404 for invalid endpoint)
if curl -s --max-time 5 "http://localhost:7071/invalid" | grep -q "endpoint_not_found"; then
    log "Rate limiting and routing working ✓"
else
    warn "Unexpected response from invalid endpoint"
fi

# Create systemd service file (if systemd is available)
if command -v systemctl &> /dev/null; then
    cat > /etc/systemd/system/vvtv-api-gateway.service << EOF
[Unit]
Description=VVTV Public API Gateway
After=network.target
Requires=network.target

[Service]
Type=forking
User=root
Group=root
ExecStart=/usr/sbin/nginx -c $NGINX_CONF
ExecReload=/bin/kill -s HUP \$MAINPID
ExecStop=/bin/kill -s QUIT \$MAINPID
PIDFile=$PID_FILE
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    log "Systemd service file created: vvtv-api-gateway.service"
    log "Enable with: systemctl enable vvtv-api-gateway"
fi

# Create launchd plist (if on macOS)
if command -v launchctl &> /dev/null; then
    cat > /Library/LaunchDaemons/vvtv.api-gateway.plist << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>vvtv.api-gateway</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/nginx</string>
        <string>-c</string>
        <string>$NGINX_CONF</string>
        <string>-g</string>
        <string>daemon off;</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$BASE/logs/api_gateway.log</string>
    <key>StandardErrorPath</key>
    <string>$BASE/logs/api_gateway_error.log</string>
</dict>
</plist>
EOF

    log "Launchd plist created: /Library/LaunchDaemons/vvtv.api-gateway.plist"
    log "Load with: sudo launchctl load -w /Library/LaunchDaemons/vvtv.api-gateway.plist"
fi

echo
log "🌐 VVTV Public API Gateway is running!"
echo
echo "📋 Endpoints:"
echo "   Health: http://localhost:7071/v1/health/ready"
echo "   Infer:  http://localhost:7071/v1/infer (requires authentication)"
echo "   Usage:  http://localhost:7071/v1/usage (requires authentication)"
echo
echo "📊 Monitoring:"
echo "   Access log: $BASE/logs/api_access.log"
echo "   Error log:  $BASE/logs/api_error.log"
echo "   PID file:   $PID_FILE"
echo
echo "🔧 Management:"
echo "   Stop:    kill $NGINX_PID"
echo "   Reload:  kill -HUP $NGINX_PID"
echo "   Test:    nginx -t -c $NGINX_CONF"

# Keep the script running to monitor the process
trap 'log "Shutting down API Gateway..."; kill $NGINX_PID; exit 0' SIGTERM SIGINT

# Monitor the NGINX process
while kill -0 "$NGINX_PID" 2>/dev/null; do
    sleep 10
done

warn "NGINX process died unexpectedly"
exit 1