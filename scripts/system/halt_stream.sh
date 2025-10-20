#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--force]

Interrompe o broadcast de forma segura, finalizando ingestões, aguardando flush e parando o serviço NGINX.
USAGE
  exit 1
}

FORCE=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)
      FORCE=true
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

log "Solicitando halt do stream"

if pgrep -f "ffmpeg.*rtmp" >/dev/null 2>&1; then
  log "Encerrando processos FFmpeg"
  pkill -15 -f "ffmpeg.*rtmp" || true
  sleep 5
  if pgrep -f "ffmpeg.*rtmp" >/dev/null 2>&1; then
    if $FORCE; then
      log "Forçando encerramento do FFmpeg"
      pkill -9 -f "ffmpeg.*rtmp" || true
    else
      log "[WARN] FFmpeg ainda ativo. Reexecute com --force se necessário."
    fi
  fi
else
  log "Nenhum processo FFmpeg identificado"
fi

if command -v systemctl >/dev/null 2>&1; then
  log "Parando serviço vvtv-nginx"
  sudo systemctl stop vvtv-nginx.service || true
elif command -v nginx >/dev/null 2>&1; then
  log "Enviando sinal QUIT ao NGINX"
  sudo nginx -s quit || true
fi

log "Limpeza de segmentos temporários"
find /vvtv/broadcast/hls -type f -mmin +180 -delete 2>/dev/null || true

log "Stream interrompido"
