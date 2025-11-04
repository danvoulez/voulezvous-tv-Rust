#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--verify]

Cria a hierarquia /vvtv, usuário vvtv (UID 9001) e aplica permissões padrões.
USAGE
  exit 1
}

VERIFY=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verify)
      VERIFY=true
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

VVTV_UID=9001
VVTV_USER=vvtv
BASE_DIR=/vvtv
LOG_FILE="$BASE_DIR/system/logs/provision_directories.log"

ensure_user() {
  if id -u "$VVTV_USER" >/dev/null 2>&1; then
    existing_uid=$(id -u "$VVTV_USER")
    if [[ $existing_uid -ne $VVTV_UID ]]; then
      echo "[WARN] Usuário $VVTV_USER existe com UID $existing_uid. Verifique manualmente." >&2
    fi
    return
  fi

  case "$(uname -s)" in
    Linux)
      sudo useradd --uid "$VVTV_UID" --system --home "$BASE_DIR" --shell /bin/bash "$VVTV_USER" || true
      ;;
    Darwin)
      sudo dscl . -create /Users/$VVTV_USER
      sudo dscl . -create /Users/$VVTV_USER UserShell /bin/zsh
      sudo dscl . -create /Users/$VVTV_USER UniqueID $VVTV_UID
      sudo dscl . -create /Users/$VVTV_USER PrimaryGroupID 20
      sudo dscl . -create /Users/$VVTV_USER NFSHomeDirectory $BASE_DIR
      sudo dscl . -passwd /Users/$VVTV_USER "$(uuidgen)"
      ;;
    *)
      echo "[ERROR] Sistema operacional não suportado para criação do usuário." >&2
      exit 1
      ;;
  esac
}

create_dirs() {
  sudo mkdir -p \
    $BASE_DIR/system/{bin,logs,logrotate,watchdog} \
    $BASE_DIR/{data,cache,storage,broadcast,monitor,vault,docs} \
    $BASE_DIR/cache/{browser_profiles,tmp_downloads,ffmpeg_tmp} \
    $BASE_DIR/storage/{ready,edited,archive} \
    $BASE_DIR/broadcast/{hls,vod} \
    $BASE_DIR/monitor/{captures,reports}
}

set_permissions() {
  sudo chown -R $VVTV_USER:$VVTV_USER $BASE_DIR
  sudo chmod 755 $BASE_DIR/system/bin
  sudo chmod 750 $BASE_DIR/system/logs
  sudo chmod 750 $BASE_DIR/system/watchdog
  sudo chmod 755 $BASE_DIR/broadcast
  sudo chmod 755 $BASE_DIR/broadcast/hls $BASE_DIR/broadcast/vod
  sudo chmod 700 $BASE_DIR/vault
  sudo chmod 770 $BASE_DIR/monitor/captures
  sudo chmod 770 $BASE_DIR/monitor/reports
  sudo chmod 755 $BASE_DIR/docs
  sudo chmod 755 $BASE_DIR/cache
  sudo chmod 755 $BASE_DIR/storage
  sudo chmod 755 $BASE_DIR/system/logrotate
}

link_logrotate() {
  if [[ -f "$(dirname "$0")/../../configs/logrotate/vvtv" ]]; then
    sudo ln -sf "$(cd "$(dirname "$0")/../.." && pwd)/configs/logrotate/vvtv" $BASE_DIR/system/logrotate/vvtv
  fi
}

verify_permissions() {
  declare -A EXPECTED=(
    ["$BASE_DIR/system/bin"]=755
    ["$BASE_DIR/system/logs"]=750
    ["$BASE_DIR/vault"]=700
    ["$BASE_DIR/broadcast/hls"]=755
    ["$BASE_DIR/data"]=755
  )
  for path in "${!EXPECTED[@]}"; do
    if [[ ! -d $path ]]; then
      echo "[ERROR] Diretório ausente: $path" >&2
      exit 1
    fi
    perms=$(stat -c %a "$path" 2>/dev/null || stat -f %Mp%Lp "$path")
    if [[ $perms != "${EXPECTED[$path]}" ]]; then
      echo "[WARN] Permissão incorreta em $path (esperado ${EXPECTED[$path]}, atual $perms)" >&2
    else
      echo "[OK] $path -> $perms"
    fi
  done
}

mkdir -p $(dirname "$LOG_FILE") || true
exec > >(tee -a "$LOG_FILE") 2>&1

echo "[INFO] $(date --iso-8601=seconds 2>/dev/null || date) - Provisionamento /vvtv"

if $VERIFY; then
  verify_permissions
  exit 0
fi

ensure_user
echo "[INFO] Usuário $VVTV_USER preparado"
create_dirs
echo "[INFO] Diretórios criados"
set_permissions
echo "[INFO] Permissões aplicadas"
link_logrotate
echo "[INFO] Logrotate referenciado"
verify_permissions

echo "[INFO] Provisionamento concluído"
