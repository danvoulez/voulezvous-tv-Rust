# Helper functions for structured incident logging.

incident_log_file() {
  if [[ -n "${INCIDENT_LOG_FILE:-}" ]]; then
    printf '%s' "$INCIDENT_LOG_FILE"
  else
    printf '%s' "/vvtv/system/logs/incident_log.md"
  fi
}

incident_log_escape() {
  local value="$1"
  value=${value//\\/\\\\}
  value=${value//|/\|}
  value=${value//$'\n'/<br/>}
  printf '%s' "$value"
}

incident_log_init() {
  local file="${1:-$(incident_log_file)}"
  local dir
  dir=$(dirname "$file")
  mkdir -p "$dir"
  if [[ ! -f "$file" ]]; then
    cat >"$file" <<'LOGHEADER'
# VVTV Incident Log

| Timestamp (UTC) | Script | Status | Message | Context |
| --- | --- | --- | --- | --- |
LOGHEADER
  fi
}

incident_log_append() {
  local script_name="$1"
  local status="$2"
  local message="$3"
  local context="${4:-}"
  local file="${INCIDENT_LOG_PATH:-$(incident_log_file)}"

  incident_log_init "$file"

  local timestamp
  timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

  local safe_script safe_status safe_message safe_context
  safe_script=$(incident_log_escape "$script_name")
  safe_status=$(incident_log_escape "$status")
  safe_message=$(incident_log_escape "$message")
  safe_context=$(incident_log_escape "$context")

  printf '| %s | %s | %s | %s | %s |\n' \
    "$timestamp" "$safe_script" "$safe_status" "$safe_message" "$safe_context" >>"$file"
}
