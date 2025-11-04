#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--check-only] [--sync-only]

Replica diretórios críticos do origin para o nó Railway usando rclone.
Variáveis de ambiente suportadas:
  REMOTE           Remote rclone de destino (padrão: railway:vv_origin)
  SYNC_PATHS       Diretórios locais separados por espaço
                    (padrão: "/vvtv/broadcast/hls /vvtv/storage/ready")
  BWLIMIT_Mbps     Limite de banda (ex.: 64)
  LOG_FILE         Log JSON (padrão: /vvtv/system/logs/origin_replication.log)
  FAILOVER_SCRIPT  Script acionado quando drift > limiar
  DRIFT_THRESHOLD  Percentual máximo aceitável (padrão: 5)
USAGE
  exit 1
}

CHECK_ONLY=false
SYNC_ONLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check-only)
      CHECK_ONLY=true
      shift
      ;;
    --sync-only)
      SYNC_ONLY=true
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

REMOTE=${REMOTE:-"railway:vv_origin"}
SYNC_PATHS=${SYNC_PATHS:-"/vvtv/broadcast/hls /vvtv/storage/ready"}
BWLIMIT=${BWLIMIT_Mbps:-}
LOG_FILE=${LOG_FILE:-"/vvtv/system/logs/origin_replication.log"}
FAILOVER_SCRIPT=${FAILOVER_SCRIPT:-"/vvtv/system/promote_failover.sh"}
DRIFT_THRESHOLD=${DRIFT_THRESHOLD:-5}

log_json() {
  local json="$1"
  mkdir -p "$(dirname "$LOG_FILE")"
  printf '%s\n' "$json" >>"$LOG_FILE"
}

ts() {
  date --iso-8601=seconds 2>/dev/null || date
}

json_escape() {
  python - <<'PY'
import json
import sys
print(json.dumps(sys.stdin.read())[1:-1])
PY
}

sync_path() {
  local path="$1"
  local destination="$REMOTE/$(basename "$path")"
  local args=("sync" "$path" "$destination" "--fast-list" "--transfers" "4" "--immutable")
  [[ -n "$BWLIMIT" ]] && args+=("--bwlimit" "${BWLIMIT}M")
  if ! output=$(rclone "${args[@]}" 2>&1); then
    local path_json destination_json message_json
    path_json=$(printf '%s' "$path" | json_escape)
    destination_json=$(printf '%s' "$destination" | json_escape)
    message_json=$(printf '%s' "$output" | json_escape)
    log_json "{\"timestamp\":\"$(ts)\",\"stage\":\"sync\",\"path\":\"$path_json\",\"destination\":\"$destination_json\",\"status\":\"error\",\"message\":\"$message_json\"}"
    return 1
  fi
  local files=$(echo "$output" | grep -Eo 'Transferred:\s+[0-9]+/[0-9]+' | awk -F'/' '{print $1}' | awk '{print $2}')
  local bytes=$(echo "$output" | grep -Eo 'Transferred:.*Bytes' | awk '{print $2}' | head -n1)
  local path_json destination_json
  path_json=$(printf '%s' "$path" | json_escape)
  destination_json=$(printf '%s' "$destination" | json_escape)
  log_json "{\"timestamp\":\"$(ts)\",\"stage\":\"sync\",\"path\":\"$path_json\",\"destination\":\"$destination_json\",\"files\":${files:-0},\"bytes\":${bytes:-0}}"
}

check_path() {
  local path="$1"
  local destination="$REMOTE/$(basename "$path")"
  if ! output=$(rclone check "$path" "$destination" --size-only --one-way 2>&1); then
    local path_json destination_json message_json
    path_json=$(printf '%s' "$path" | json_escape)
    destination_json=$(printf '%s' "$destination" | json_escape)
    message_json=$(printf '%s' "$output" | json_escape)
    log_json "{\"timestamp\":\"$(ts)\",\"stage\":\"check\",\"path\":\"$path_json\",\"destination\":\"$destination_json\",\"status\":\"error\",\"message\":\"$message_json\"}"
    echo "0 0"
    return 1
  fi
  local diff=$(echo "$output" | grep -Eo '[0-9]+ differences' | awk '{print $1}' | head -n1)
  local total=$(find "$path" -type f | wc -l | awk '{print $1}')
  local path_json destination_json
  path_json=$(printf '%s' "$path" | json_escape)
  destination_json=$(printf '%s' "$destination" | json_escape)
  log_json "{\"timestamp\":\"$(ts)\",\"stage\":\"check\",\"path\":\"$path_json\",\"destination\":\"$destination_json\",\"differences\":${diff:-0},\"files\":${total:-0}}"
  echo "${diff:-0} ${total:-0}"
}

if ! $CHECK_ONLY; then
  for path in $SYNC_PATHS; do
    [[ -d "$path" ]] || continue
    sync_path "$path"
  done
fi

if ! $SYNC_ONLY; then
  total_differences=0
  total_files=0
  for path in $SYNC_PATHS; do
    [[ -d "$path" ]] || continue
    read -r diff files < <(check_path "$path")
    total_differences=$((total_differences + diff))
    total_files=$((total_files + files))
  done
  drift=0
  if [[ $total_files -gt 0 ]]; then
    drift=$(python - "$total_differences" "$total_files" <<'PY'
import sys
if int(sys.argv[2]) == 0:
    print("0.00")
else:
    diff = int(sys.argv[1])
    total = int(sys.argv[2])
    print(f"{(diff/total)*100:.2f}")
PY
)
  fi
  log_json "{\"timestamp\":\"$(ts)\",\"stage\":\"summary\",\"total_differences\":$total_differences,\"total_files\":$total_files,\"drift_percent\":$drift}"
  drift_int=$(printf '%.0f' "$drift")
  if (( drift_int > DRIFT_THRESHOLD )); then
    if [[ -x "$FAILOVER_SCRIPT" ]]; then
      "$FAILOVER_SCRIPT" --reason "replication-drift" --percent "$drift"
    fi
  fi
fi
