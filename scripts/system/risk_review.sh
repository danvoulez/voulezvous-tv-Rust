#!/usr/bin/env bash
# VVTV Risk Review — consulta matriz de riscos e avalia gatilhos operacionais

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/.."
# shellcheck source=./lib/incident_logging.sh
source "$SCRIPT_DIR/lib/incident_logging.sh"

VVTV_BASE_DIR=${VVTV_BASE_DIR:-/vvtv}
CONFIG_FILE=${RISK_REGISTER_FILE:-"$ROOT_DIR/configs/risk_register.json"}
OUTPUT_JSON=0

usage() {
  cat <<'USAGE'
Usage: risk_review.sh [--json] [--config PATH]

Options:
  --config PATH  Caminho alternativo para risk_register.json
  --json         Emite resultado em JSON (stdout)
  --help         Exibe esta mensagem
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)
      CONFIG_FILE="$2"
      shift 2
      ;;
    --json)
      OUTPUT_JSON=1
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

if [[ ! -f "$CONFIG_FILE" ]]; then
  echo "Risk register não encontrado em $CONFIG_FILE" >&2
  incident_log_append "risk_review.sh" "error" "risk_register ausente" "$CONFIG_FILE"
  exit 2
fi

to_json_array() {
  if [[ $# -eq 0 ]]; then
    printf '[]'
  else
    python3 - "$@" <<'PY'
import json, sys
print(json.dumps(sys.argv[1:]))
PY
  fi
}

json_query() {
  local path="$1"
  python3 - "$CONFIG_FILE" "$path" <<'PY'
import json, sys
config = json.load(open(sys.argv[1], 'r', encoding='utf-8'))
path = sys.argv[2]
parts = path.strip('/').split('/') if path else []
node = config
for part in parts:
    if isinstance(node, list):
        part = int(part)
    node = node[part]
print(json.dumps(node))
PY
}

printf '==== VVTV RISK REGISTER REVIEW ====%s' "\n"
printf 'Fonte: %s\n' "$CONFIG_FILE"

RISKS=$(json_query "risks")
python3 - <<PY "$RISKS"
import json, sys
risks = json.loads(sys.argv[1])
print("\nID  Prob.  Impacto  Dono")
print("--  -----  -------  ----")
for risk in risks:
    print(f"{risk['id']:<3} {risk['probability']:<6} {risk['impact']:<8} {risk['owner']}")
PY

alerts=()
metrics_summary=()

queue_db="$VVTV_BASE_DIR/data/queue.sqlite"
metrics_db="$VVTV_BASE_DIR/data/metrics.sqlite"

trigger_eval() {
  local risk_id="$1"
  local description="$2"
  local source="$3"
  local metric="$4"
  local threshold="$5"
  local comparison="$6"
  local action="$7"
  local value=""

  case "$source" in
    queue.sqlite)
      if [[ -f "$queue_db" ]]; then
        value=$(sqlite3 "$queue_db" "SELECT COALESCE(SUM(duration_s),0)/3600.0 FROM playout_queue WHERE status='queued';")
      else
        value=""
      fi
      ;;
    metrics.sqlite)
      if [[ -f "$metrics_db" ]]; then
        value=$(sqlite3 "$metrics_db" "SELECT $metric FROM metrics ORDER BY ts DESC LIMIT 1;")
      else
        value=""
      fi
      ;;
  esac

  if [[ -z "$value" ]]; then
    metrics_summary+=("$risk_id: sem dados ($description)")
    return
  fi

  metrics_summary+=("$risk_id: $metric=$value ($description)")

  python3 - "$value" "$threshold" "$comparison" <<'PY'
import sys
value = float(sys.argv[1])
threshold = float(sys.argv[2])
comparison = sys.argv[3]
if comparison == '<' and value < threshold:
    sys.exit(0)
elif comparison == '>' and value > threshold:
    sys.exit(0)
elif comparison == '=' and value == threshold:
    sys.exit(0)
sys.exit(1)
PY
  local triggered=$?
  if [[ $triggered -eq 0 ]]; then
    alerts+=("$risk_id: $description → $action (valor=$value)")
  fi
}

TRIGGERS=$(json_query "metric_triggers")
python3 - <<'PY' "$TRIGGERS"
import json, sys
for idx, trigger in enumerate(json.loads(sys.argv[1])):
    print(f"{idx}|{trigger['risk_id']}|{trigger['description']}|{trigger['source']}|{trigger['metric']}|{trigger['threshold']}|{trigger['comparison']}|{trigger['action']}")
PY | while IFS='|' read -r idx risk_id description source metric threshold comparison action; do
  trigger_eval "$risk_id" "$description" "$source" "$metric" "$threshold" "$comparison" "$action"
done

printf '\n-- Métricas monitoradas --\n'
for item in "${metrics_summary[@]}"; do
  printf '* %s\n' "$item"
done

if [[ ${#alerts[@]} -gt 0 ]]; then
  printf '\n⚠️  ALERTAS ATIVOS\n'
  for alert in "${alerts[@]}"; do
    printf '! %s\n' "$alert"
  done
else
  printf '\n✅ Nenhum gatilho crítico disparado.\n'
fi

REVIEWS=$(json_query "review_schedule")
python3 - <<'PY' "$REVIEWS"
import json, sys
reviews = json.loads(sys.argv[1])
print("\n-- Agenda de Revisão --")
for item in reviews:
    print(f"- {item['frequency']}: {item['action']} → {item['owner']} ({item['deliverable']})")
PY

status="ok"
if [[ ${#alerts[@]} -gt 0 ]]; then
  status="alert"
fi
incident_log_append "risk_review.sh" "$status" "revisão de riscos" "alerts=${#alerts[@]}"

if [[ $OUTPUT_JSON -eq 1 ]]; then
  alerts_json=$(to_json_array "${alerts[@]}")
  metrics_json=$(to_json_array "${metrics_summary[@]}")
  ALERTS_JSON="$alerts_json" METRICS_JSON="$metrics_json" STATUS="$status" python3 - <<'PY'
import json, os
print(json.dumps({
  "status": os.environ["STATUS"],
  "alerts": json.loads(os.environ["ALERTS_JSON"]),
  "metrics": json.loads(os.environ["METRICS_JSON"])
}))
PY
fi
