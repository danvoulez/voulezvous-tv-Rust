#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--output <path>] [--append]

Gera um relatório de inventário de hardware e condições ambientais conforme PR A1.
Por padrão o arquivo é salvo em /vvtv/system/logs/hardware_inventory_<timestamp>.md.
USAGE
  exit 1
}

OUTPUT=""
APPEND=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      OUTPUT="$2"
      shift 2
      ;;
    --append)
      APPEND=true
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

LOG_DIR="/vvtv/system/logs"
if [[ ! -d $LOG_DIR ]]; then
  sudo mkdir -p "$LOG_DIR"
  sudo chown vvtv:"${SUDO_GID:-vvtv}" "$LOG_DIR" 2>/dev/null || true
fi

if [[ -z $OUTPUT ]]; then
  timestamp=$(date +"%Y%m%d_%H%M%S")
  OUTPUT="$LOG_DIR/hardware_inventory_${timestamp}.md"
fi

tmp_report=$(mktemp)
trap 'rm -f "$tmp_report"' EXIT

section() {
  echo "## $1" >>"$tmp_report"
}

echo "# VVTV Hardware & Environment Report" >>"$tmp_report"
echo "Gerado em $(date --iso-8601=seconds 2>/dev/null || date)" >>"$tmp_report"
echo >>"$tmp_report"

section "Sistema Operacional"
echo '\n```' >>"$tmp_report"
uname -a >>"$tmp_report"
echo '```' >>"$tmp_report"

if command -v sw_vers >/dev/null 2>&1; then
  echo '\n### macOS' >>"$tmp_report"
  echo '\n```' >>"$tmp_report"
  sw_vers >>"$tmp_report"
  echo '```' >>"$tmp_report"
elif [[ -f /etc/os-release ]]; then
  echo '\n### Linux' >>"$tmp_report"
  echo '\n```' >>"$tmp_report"
  cat /etc/os-release >>"$tmp_report"
  echo '```' >>"$tmp_report"
fi

section "CPU & Memória"
if command -v lscpu >/dev/null 2>&1; then
  echo '\n```' >>"$tmp_report"
  lscpu >>"$tmp_report"
  echo '```' >>"$tmp_report"
elif command -v sysctl >/dev/null 2>&1; then
  echo '\n```' >>"$tmp_report"
  sysctl -n machdep.cpu.brand_string >>"$tmp_report"
  sysctl hw.ncpu hw.memsize >>"$tmp_report"
  echo '```' >>"$tmp_report"
fi

section "Armazenamento"
if command -v lsblk >/dev/null 2>&1; then
  echo '\n```' >>"$tmp_report"
  lsblk -o NAME,MODEL,SIZE,MOUNTPOINT >>"$tmp_report"
  echo '```' >>"$tmp_report"
elif command -v diskutil >/dev/null 2>&1; then
  echo '\n```' >>"$tmp_report"
  diskutil list >>"$tmp_report"
  echo '```' >>"$tmp_report"
fi

section "Temperatura & Umidade"
if command -v sensors >/dev/null 2>&1; then
  echo '\n```' >>"$tmp_report"
  sensors >>"$tmp_report" || true
  echo '```' >>"$tmp_report"
else
  echo "\n> Informe manualmente os valores coletados do sensor externo." >>"$tmp_report"
fi

section "Energia"
if command -v upower >/dev/null 2>&1; then
  echo '\n```' >>"$tmp_report"
  upower -e | while read -r dev; do
    upower -i "$dev" | grep -E 'device|model|power supply|percentage'
  done >>"$tmp_report" || true
  echo '```' >>"$tmp_report"
else
  echo "\n> Registre manualmente o status da UPS (modelo, capacidade, tempo de autonomia)." >>"$tmp_report"
fi

section "Observações"
echo "\n- Operador:" >>"$tmp_report"
echo "- Sensor ambiental utilizado:" >>"$tmp_report"
echo "- Fotos anexas: /vvtv/vault/layout/" >>"$tmp_report"

if $APPEND && [[ -f $OUTPUT ]]; then
  cat "$tmp_report" >>"$OUTPUT"
else
  mv "$tmp_report" "$OUTPUT"
fi

echo "Relatório gerado em $OUTPUT"
