#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--with-firewall] [--harden] [--register-healthcron]

Instala dependências base do VVTV (FFmpeg, SQLite, NGINX-RTMP, aria2, Chromium, Rust, Tailscale).
USAGE
  exit 1
}

WITH_FIREWALL=false
HARDEN=false
REGISTER_HEALTHCRON=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-firewall)
      WITH_FIREWALL=true
      shift
      ;;
    --harden)
      HARDEN=true
      shift
      ;;
    --register-healthcron)
      REGISTER_HEALTHCRON=true
      shift
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "Opção desconhecida: $1" >&2
      usage
      ;;
  esac
done

log() {
  echo "[$(date --iso-8601=seconds 2>/dev/null || date)] $*"
}

LOG_FILE="/vvtv/system/logs/install_base_stack.log"
mkdir -p /vvtv/system/logs >/dev/null 2>&1 || true
exec > >(tee -a "$LOG_FILE") 2>&1

install_linux() {
  log "Atualizando pacotes"
  sudo apt update
  sudo apt install -y ffmpeg aria2 sqlite3 nginx-full libnginx-mod-rtmp chromium build-essential curl git pkg-config

  if ! command -v tailscale >/dev/null 2>&1; then
    curl -fsSL https://tailscale.com/install.sh | sh
  fi

  if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
  fi
}

install_macos() {
  if ! command -v brew >/dev/null 2>&1; then
    log "Homebrew não detectado"
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  fi

  brew update
  brew install ffmpeg aria2 sqlite nginx-full chromium tailscale
  brew install --cask chromium || true

  if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
  fi
}

verify_versions() {
  log "Verificando versões"
  ffmpeg -version | head -n1
  sqlite3 --version
  nginx -V 2>&1 | grep --color=never rtmp || log "[WARN] Módulo RTMP não detectado"
  rustc --version || log "[WARN] Rust não disponível"
  tailscale version || log "[WARN] Tailscale não disponível"
}

configure_firewall() {
  case "$(uname -s)" in
    Linux)
      if command -v ufw >/dev/null 2>&1; then
        log "Configurando ufw"
        sudo ufw allow 22/tcp
        sudo ufw allow 1935/tcp
        sudo ufw allow 8080/tcp
        sudo ufw --force enable
      else
        log "[WARN] ufw não encontrado, configure firewall manualmente"
      fi
      ;;
    Darwin)
      if command -v pfctl >/dev/null 2>&1; then
        cat <<PF >/tmp/vvtv_pf.conf
block in on en0 all
pass in on en0 proto tcp from any to any port {22,1935,8080}
pass out on en0 all
PF
        sudo pfctl -f /tmp/vvtv_pf.conf
        sudo pfctl -e || true
      else
        log "[WARN] pfctl indisponível"
      fi
      ;;
  esac
}

apply_hardening() {
  case "$(uname -s)" in
    Linux)
      log "Aplicando hardening Linux"
      sudo systemctl mask sleep.target suspend.target hibernate.target hybrid-sleep.target || true
      ;;
    Darwin)
      log "Aplicando hardening macOS"
      sudo pmset -a sleep 0 displaysleep 0 disksleep 0 || true
      sudo tmutil disable || true
      sudo mdutil -a -i off || true
      ;;
  esac
}

register_healthcron() {
  if [[ ! -x /vvtv/system/bin/check_stream_health.sh ]]; then
    log "[WARN] check_stream_health.sh não encontrado em /vvtv/system/bin"
    return
  fi
  tmp_cron=$(mktemp)
  sudo crontab -l 2>/dev/null >"$tmp_cron" || true
  if ! grep -q 'check_stream_health.sh' "$tmp_cron"; then
    echo "*/5 * * * * /vvtv/system/bin/check_stream_health.sh --quiet" >>"$tmp_cron"
    sudo crontab "$tmp_cron"
    log "Crontab atualizado"
  else
    log "Crontab já contém check_stream_health.sh"
  fi
  rm -f "$tmp_cron"
}

case "$(uname -s)" in
  Linux)
    install_linux
    ;;
  Darwin)
    install_macos
    ;;
  *)
    log "Sistema não suportado"
    exit 1
    ;;
 esac

verify_versions

if $WITH_FIREWALL; then
  configure_firewall
fi

if $HARDEN; then
  apply_hardening
fi

if $REGISTER_HEALTHCRON; then
  register_healthcron
fi

log "Instalação concluída"
