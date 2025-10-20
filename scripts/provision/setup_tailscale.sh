#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--hostname <name>] [--auth-key <key>] [--advertise-tags <tags>] [--verify-only] [--harden]

Instala e configura Tailscale conforme padrão VVTV. Utilize --verify-only para apenas validar o estado atual.
USAGE
  exit 1
}

HOSTNAME=""
AUTH_KEY=""
ADVERTISE_TAGS=""
VERIFY_ONLY=false
HARDEN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --hostname)
      HOSTNAME="$2"
      shift 2
      ;;
    --auth-key)
      AUTH_KEY="$2"
      shift 2
      ;;
    --advertise-tags)
      ADVERTISE_TAGS="$2"
      shift 2
      ;;
    --verify-only)
      VERIFY_ONLY=true
      shift
      ;;
    --harden)
      HARDEN=true
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

LOG_FILE="/vvtv/system/logs/tailscale_setup.log"
mkdir -p /vvtv/system/logs >/dev/null 2>&1 || true
exec > >(tee -a "$LOG_FILE") 2>&1

echo "[INFO] $(date --iso-8601=seconds 2>/dev/null || date) - Iniciando setup Tailscale"

if ! $VERIFY_ONLY; then
  if [[ -z $AUTH_KEY ]]; then
    echo "[ERROR] --auth-key é obrigatório durante o provisionamento" >&2
    exit 1
  fi

  if ! command -v tailscale >/dev/null 2>&1; then
    echo "[INFO] Instalando Tailscale"
    case "$(uname -s)" in
      Darwin)
        if ! command -v brew >/dev/null 2>&1; then
          echo "[ERROR] Homebrew não encontrado. Instale antes de continuar." >&2
          exit 1
        fi
        brew install tailscale
        ;;
      Linux)
        if command -v apt >/dev/null 2>&1; then
          curl -fsSL https://tailscale.com/install.sh | sh
        elif command -v yum >/dev/null 2>&1; then
          curl -fsSL https://tailscale.com/install.sh | sh
        else
          echo "[ERROR] Distribuição não suportada automaticamente." >&2
          exit 1
        fi
        ;;
      *)
        echo "[ERROR] Sistema operacional não suportado." >&2
        exit 1
        ;;
    esac
  fi

  if [[ -n $HOSTNAME ]]; then
    tailscale set --hostname "$HOSTNAME" || true
  fi

  cmd=(tailscale up --auth-key "$AUTH_KEY")
  if [[ -n $ADVERTISE_TAGS ]]; then
    cmd+=(--advertise-tags "$ADVERTISE_TAGS")
  fi
  if $HARDEN; then
    cmd+=(--ssh=false --accept-routes=true --reset)
  fi

  echo "[INFO] Executando: ${cmd[*]}"
  "${cmd[@]}"

  echo "[INFO] Ajustando ACL local"
  mkdir -p /etc/tailscale 2>/dev/null || true
  cat <<ACL >/etc/tailscale/tailnet_policy_hint.json
{
  "allow-rdp": false,
  "allow-ssh": true,
  "required-ports": [22, 80, 1935, 8080]
}
ACL
fi

echo "[INFO] Verificando estado atual"
tailscale status || { echo "[ERROR] tailscale status falhou" >&2; exit 1; }

echo "[INFO] IPs Tailscale"
tailscale ip -4 || true

echo "[INFO] Testando conectividade"
if command -v nc >/dev/null 2>&1; then
  for port in 22 1935 8080; do
    nc -z localhost "$port" >/dev/null 2>&1 && echo "[INFO] Porta $port aberta localmente"
  done
fi

echo "[INFO] Setup Tailscale concluído"
