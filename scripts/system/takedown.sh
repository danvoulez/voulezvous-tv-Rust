#!/usr/bin/env bash
# VVTV Legal takedown — removes an asset and updates ledgers

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
PLANS_DB=${PLANS_DB:-"$VVTV_BASE_DIR/data/plans.sqlite"}
QUEUE_DB=${QUEUE_DB:-"$VVTV_BASE_DIR/data/queue.sqlite"}
READY_DIR=${READY_DIR:-"$VVTV_BASE_DIR/storage/ready"}
QUARANTINE_DIR=${QUARANTINE_DIR:-"$VVTV_BASE_DIR/storage/quarantine"}
DRY_RUN=0
PLAN_ID=""
REASON="DMCA takedown"

usage() {
  cat <<'USAGE'
Usage: $(basename "$0") --id PLAN_ID [--reason TEXTO] [--dry-run]

Options:
  --id PLAN_ID   Identificador do plano/asset a remover (obrigatório)
  --reason TXT   Motivo registrado no log (padrão: DMCA takedown)
  --dry-run      Apenas exibe ações sem alterar estado
  --help         Exibe esta mensagem
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --id)
      PLAN_ID="$2"
      shift 2
      ;;
    --reason)
      REASON="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
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

if [[ -z "$PLAN_ID" ]]; then
  echo "--id é obrigatório" >&2
  usage >&2
  exit 1
fi

require_db() {
  if [[ ! -f "$1" ]]; then
    echo "Banco ausente: $1" >&2
    incident_log_append "takedown.sh" "error" "banco ausente" "$1"
    exit 2
  fi
}

require_db "$PLANS_DB"
require_db "$QUEUE_DB"

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 não encontrado" >&2
  incident_log_append "takedown.sh" "error" "sqlite3 ausente" "PATH=$PATH"
  exit 2
fi

sql_escape() {
  printf "%s" "$1" | sed "s/'/''/g"
}

status_before=$(sqlite3 "$PLANS_DB" "SELECT status FROM plans WHERE plan_id='$(sql_escape "$PLAN_ID")';")
if [[ -z "$status_before" ]]; then
  echo "Plan $PLAN_ID não encontrado em plans.sqlite" >&2
  incident_log_append "takedown.sh" "warning" "plan inexistente" "$PLAN_ID"
  exit 1
fi

asset_path=$(sqlite3 "$QUEUE_DB" "SELECT asset_path FROM playout_queue WHERE plan_id='$(sql_escape "$PLAN_ID")' ORDER BY id DESC LIMIT 1;")
if [[ -z "$asset_path" ]]; then
  asset_path="$READY_DIR/$PLAN_ID"
fi

move_target=""
if [[ -e "$asset_path" ]]; then
  mkdir -p "$QUARANTINE_DIR"
  timestamp=$(date -u +%Y%m%dT%H%M%SZ)
  base_name=$(basename "$asset_path")
  move_target="$QUARANTINE_DIR/${PLAN_ID}_${timestamp}_${base_name}"
fi

if [[ $DRY_RUN -eq 0 ]]; then
  sqlite3 "$PLANS_DB" <<SQL
BEGIN;
UPDATE plans SET status='takedown', updated_at=CURRENT_TIMESTAMP WHERE plan_id='$(sql_escape "$PLAN_ID")';
INSERT INTO plan_attempts (plan_id, status_from, status_to, note)
VALUES ('$(sql_escape "$PLAN_ID")', '$(sql_escape "$status_before")', 'takedown', 'Takedown: $(sql_escape "$REASON")');
COMMIT;
SQL

  sqlite3 "$QUEUE_DB" "DELETE FROM playout_queue WHERE plan_id='$(sql_escape "$PLAN_ID")';"

  if [[ -n "$move_target" ]]; then
    if [[ -e "$move_target" ]]; then
      move_target+="_dup"
    fi
    mv "$asset_path" "$move_target"
  fi
else
  cat <<INFO
[DRY-RUN] Atualizaria status de $PLAN_ID para takedown (antes: $status_before)
[DRY-RUN] Removeria entradas da fila
INFO
  if [[ -n "$move_target" ]]; then
    echo "[DRY-RUN] Moveria $asset_path -> $move_target"
  else
    echo "[DRY-RUN] Sem arquivos para mover"
  fi
fi

summary="reason=$REASON status_before=$status_before dry_run=$DRY_RUN"
incident_log_append "takedown.sh" "$( [[ $DRY_RUN -eq 1 ]] && echo dry-run || echo success )" "takedown $PLAN_ID" "$summary"

echo "Takedown concluído para $PLAN_ID"
if [[ -n "$move_target" ]]; then
  echo "Asset movido para $move_target"
fi
