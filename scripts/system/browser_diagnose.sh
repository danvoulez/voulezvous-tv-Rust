#!/usr/bin/env bash
# VVTV Browser Diagnose — gathers signals about curator browser health

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
PROFILE_ROOT=${PROFILE_ROOT:-"$VVTV_BASE_DIR/cache/browser_profiles"}
LOG_ROOT=${CURATOR_LOG_DIR:-"$VVTV_BASE_DIR/system/logs/curator"}
CONFIG_FILE=${CONFIG_FILE:-"$(cd "$SCRIPT_DIR/.." && pwd)/../configs/browser.toml"}
PROFILE_ID=""
JSON=0

usage() {
  cat <<'USAGE'
Usage: $(basename "$0") [--profile ID] [--json]

Options:
  --profile ID  Analisa apenas o perfil especificado
  --json        Emitir resumo final em JSON
  --help        Exibe esta mensagem
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE_ID="$2"
      shift 2
      ;;
    --json)
      JSON=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Opção desconhecida: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

section() {
  printf '\n%s\n' "==== $1 ===="
}

WARNINGS=()
INFO=()

section "Processos ativos"
if pgrep -fl 'chrom(ium|e).*--remote-debugging-port' >/dev/null 2>&1; then
  pgrep -fl 'chrom(ium|e).*--remote-debugging-port'
  INFO+=("process=present")
else
  echo "Nenhum processo Chromium com remote-debugging detectado"
  WARNINGS+=("browser_process_missing")
fi

section "Perfis"
if [[ -d "$PROFILE_ROOT" ]]; then
  if [[ -n "$PROFILE_ID" ]]; then
    TARGET="$PROFILE_ROOT/$PROFILE_ID"
    if [[ -d "$TARGET" ]]; then
      du -sh "$TARGET" 2>/dev/null || ls "$TARGET"
      INFO+=("profile=$PROFILE_ID")
    else
      echo "Perfil $PROFILE_ID não encontrado em $PROFILE_ROOT"
      WARNINGS+=("profile_missing:$PROFILE_ID")
    fi
  else
    ls -1 "$PROFILE_ROOT" 2>/dev/null || true
    COUNT=$(find "$PROFILE_ROOT" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')
    if [[ "$COUNT" -eq 0 ]]; then
      echo "Nenhum perfil ativo"
      WARNINGS+=("no_profiles")
    else
      INFO+=("profiles=$COUNT")
    fi
  fi
else
  echo "Diretório de perfis ausente: $PROFILE_ROOT"
  WARNINGS+=("profile_root_missing")
fi

section "Proxy e fingerprints"
if [[ -f "$CONFIG_FILE" ]]; then
  grep -E '^[[:space:]]*(proxy_|fingerprint|user_agent)' "$CONFIG_FILE" || true
  ACTIVE_PROXIES=$(grep -E '^[[:space:]]*proxy_servers' "$CONFIG_FILE" | sed 's/.*=//')
  if [[ -z "${ACTIVE_PROXIES// /}" ]]; then
    WARNINGS+=("proxy_pool_empty")
  else
    INFO+=("proxy_pool_configured")
  fi
else
  echo "Configuração não encontrada em $CONFIG_FILE"
  WARNINGS+=("browser_config_missing")
fi

section "Rede"
if command -v tailscale >/dev/null 2>&1; then
  tailscale status 2>/dev/null | head -n 10 || tailscale status 2>/dev/null
  INFO+=("tailscale=ok")
else
  echo "tailscale não disponível"
  WARNINGS+=("tailscale_missing")
fi

section "Últimos erros do curador"
if [[ -d "$LOG_ROOT" ]]; then
  if compgen -G "$LOG_ROOT/*.log" >/dev/null 2>&1; then
    tail -n 20 "$LOG_ROOT"/*.log 2>/dev/null || true
  else
    echo "Sem logs recentes"
  fi
else
  echo "Diretório de logs ausente: $LOG_ROOT"
  WARNINGS+=("curator_logs_missing")
fi

STATUS="ok"
if [[ ${#WARNINGS[@]} -gt 0 ]]; then
  STATUS="warning"
fi

SUMMARY="status=$STATUS warnings=${#WARNINGS[@]} profile=${PROFILE_ID:-all}"
incident_log_append "browser_diagnose.sh" "$STATUS" "diagnóstico navegador" "$SUMMARY"

if [[ $JSON -eq 1 ]]; then
  python3 - <<PY
import json
print(json.dumps({
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "status": "$STATUS",
    "warnings": ${#WARNINGS[@]},
    "profile": "${PROFILE_ID:-all}",
    "info_count": ${#INFO[@]}
}))
PY
fi
