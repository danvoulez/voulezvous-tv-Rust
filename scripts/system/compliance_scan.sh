#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN_PATH="${VVTVCTL_BIN:-$ROOT_DIR/target/release/vvtvctl}"
CONFIG_PATH="${VVTV_CONFIG:-$ROOT_DIR/configs/vvtv.toml}"
LOGS_DIR="${VVTV_LICENSE_LOGS:-/vvtv/vault/compliance/license_logs}"
MANIFESTS_DIR="${VVTV_MANIFESTS:-/vvtv/broadcast/hls}"
MEDIA_DIR="${VVTV_MEDIA:-/vvtv/storage/ready}"
HASH_DB="${VVTV_HASH_DB:-/vvtv/vault/compliance/csam/hashes.csv}"
OUTPUT_DIR="${VVTV_COMPLIANCE_OUT:-/vvtv/system/logs}"

mkdir -p "$OUTPUT_DIR"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
JSON_REPORT="$OUTPUT_DIR/compliance_${TIMESTAMP}.json"
TEXT_REPORT="$OUTPUT_DIR/compliance_${TIMESTAMP}.log"

if [[ ! -x "$BIN_PATH" ]]; then
    echo "Erro: binário vvtvctl não encontrado em $BIN_PATH" >&2
    exit 1
fi

set +e
"$BIN_PATH" \
    --config "$CONFIG_PATH" \
    compliance suite \
    --logs-dir "$LOGS_DIR" \
    --manifests-dir "$MANIFESTS_DIR" \
    --media-dir "$MEDIA_DIR" \
    --hash-db "$HASH_DB" \
    --format json | tee "$JSON_REPORT"
STATUS=$?
set -e

if [[ $STATUS -ne 0 ]]; then
    echo "Varredura de compliance falhou (exit code $STATUS)" >&2
    exit $STATUS
fi

"$BIN_PATH" \
    --config "$CONFIG_PATH" \
    compliance suite \
    --logs-dir "$LOGS_DIR" \
    --manifests-dir "$MANIFESTS_DIR" \
    --media-dir "$MEDIA_DIR" \
    --hash-db "$HASH_DB" \
    --format text > "$TEXT_REPORT"

echo "Relatórios salvos em:" >&2
echo "  - $JSON_REPORT" >&2
echo "  - $TEXT_REPORT" >&2
