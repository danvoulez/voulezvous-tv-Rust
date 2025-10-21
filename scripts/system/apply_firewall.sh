#!/bin/bash
# VVTV firewall rules bootstrap

set -euo pipefail

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
LOG_FILE=${VVTV_SECURITY_LOG:-$VVTV_BASE_DIR/system/logs/security.log}
TAILSCALE_IF=${TAILSCALE_INTERFACE:-$(ip -o link show | awk -F': ' '/tailscale/ {print $2; exit}')}

mkdir -p "$(dirname "$LOG_FILE")"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [firewall] $*" | tee -a "$LOG_FILE"
}

apply_iptables() {
    local chain=VVTV-FW
    sudo iptables -N "$chain" 2>/dev/null || sudo iptables -F "$chain"
    sudo iptables -A "$chain" -m state --state ESTABLISHED,RELATED -j ACCEPT
    if [[ -n "$TAILSCALE_IF" ]]; then
        sudo iptables -A "$chain" -i "$TAILSCALE_IF" -p tcp --match multiport --dports 22,1935,8080 -j ACCEPT
        sudo iptables -A "$chain" -i "$TAILSCALE_IF" -p udp --dport 41641 -j ACCEPT
    fi
    sudo iptables -A "$chain" -s 127.0.0.0/8 -j ACCEPT
    sudo iptables -A "$chain" -j LOG --log-prefix "VVTV-FW DROP "
    sudo iptables -A "$chain" -j DROP
    sudo iptables -C INPUT -j "$chain" >/dev/null 2>&1 || sudo iptables -I INPUT 1 -j "$chain"
    log "iptables configurado para interface $TAILSCALE_IF"
}

apply_nftables() {
    sudo nft add table inet vvtv_firewall 2>/dev/null || true
    sudo nft flush table inet vvtv_firewall
    sudo nft add chain inet vvtv_firewall input { type filter hook input priority 0; }
    sudo nft add rule inet vvtv_firewall input ct state established,related accept
    if [[ -n "$TAILSCALE_IF" ]]; then
        sudo nft add rule inet vvtv_firewall input iif "$TAILSCALE_IF" tcp dport {22,1935,8080} accept
        sudo nft add rule inet vvtv_firewall input iif "$TAILSCALE_IF" udp dport 41641 accept
    fi
    sudo nft add rule inet vvtv_firewall input ip saddr 127.0.0.0/8 accept
    sudo nft add rule inet vvtv_firewall input log prefix "VVTV-NFT DROP " level info
    sudo nft add rule inet vvtv_firewall input drop
    log "nftables configurado"
}

if command -v nft >/dev/null 2>&1; then
    apply_nftables
elif command -v iptables >/dev/null 2>&1; then
    apply_iptables
else
    log "Nenhum backend de firewall encontrado"
fi

log "Regras aplicadas"
