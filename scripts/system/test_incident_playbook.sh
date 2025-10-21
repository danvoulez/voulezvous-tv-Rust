#!/usr/bin/env bash
# Sanity checks for incident response scripts (syntax + dry-run smoke)

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS=(
  "$ROOT_DIR/check_queue.sh"
  "$ROOT_DIR/inject_emergency_loop.sh"
  "$ROOT_DIR/browser_diagnose.sh"
  "$ROOT_DIR/takedown.sh"
  "$ROOT_DIR/restart_encoder.sh"
  "$ROOT_DIR/integrity_check.sh"
  "$ROOT_DIR/switch_cdn.sh"
)

for script in "${SCRIPTS[@]}"; do
  bash -n "$script"
  if command -v shellcheck >/dev/null 2>&1; then
    shellcheck -x "$script"
  fi
done

TMP_ROOT=$(mktemp -d)
trap 'rm -rf "$TMP_ROOT"' EXIT

BASE="$TMP_ROOT/vvtv"
mkdir -p "$BASE/data" "$BASE/storage/archive" "$BASE/system/logs/curator" \
         "$BASE/system/logs" "$BASE/system/reports" "$BASE/broadcast/hls" \
         "$BASE/cache/browser_profiles/profile-1"
touch "$BASE/broadcast/hls/live.m3u8"
touch "$BASE/system/logs/curator/session.log"

cat >"$BASE/storage/archive/sample.mp4" <<'MP4'
placeholder
MP4

sqlite3 "$BASE/data/queue.sqlite" <<'SQL'
CREATE TABLE playout_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  plan_id TEXT,
  asset_path TEXT,
  duration_s INTEGER,
  status TEXT DEFAULT 'queued',
  curation_score REAL DEFAULT 0.5,
  priority INTEGER DEFAULT 0,
  created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO playout_queue (plan_id, asset_path, duration_s, status) VALUES ('sample-plan', '/vvtv/storage/ready/sample-plan/master.mp4', 1800, 'queued');
SQL

sqlite3 "$BASE/data/plans.sqlite" <<'SQL'
CREATE TABLE plans (
  plan_id TEXT PRIMARY KEY,
  status TEXT,
  updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE plan_attempts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  plan_id TEXT,
  status_from TEXT,
  status_to TEXT,
  note TEXT,
  created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO plans (plan_id, status) VALUES ('sample-plan', 'queued');
SQL

BIN_DIR="$TMP_ROOT/bin"
mkdir -p "$BIN_DIR"
PATH="$BIN_DIR:$PATH"

cat >"$BIN_DIR/ffprobe" <<'EOF'
#!/usr/bin/env bash
echo "60.0"
EOF
chmod +x "$BIN_DIR/ffprobe"

cat >"$BIN_DIR/tailscale" <<'EOF'
#!/usr/bin/env bash
echo "100.0"
EOF
chmod +x "$BIN_DIR/tailscale"

cat >"$BIN_DIR/timedatectl" <<'EOF'
#!/usr/bin/env bash
echo "System clock synchronized: yes"
EOF
chmod +x "$BIN_DIR/timedatectl"

cat >"$BIN_DIR/ntpdate" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$BIN_DIR/ntpdate"

cat >"$BIN_DIR/pgrep" <<'EOF'
#!/usr/bin/env bash
if [[ "$*" == *nginx* ]]; then
  echo "1234"
  exit 0
fi
if [[ "$*" == *ffmpeg* ]]; then
  exit 1
fi
exit 1
EOF
chmod +x "$BIN_DIR/pgrep"

cat >"$BIN_DIR/bc" <<'EOF'
#!/usr/bin/env python3
import sys

scale = None
expr = []

for token in sys.argv[1:]:
    if token == '-l':
        continue

data = sys.stdin.read()
for line in data.splitlines():
    for chunk in line.split(';'):
        chunk = chunk.strip()
        if not chunk:
            continue
        if chunk.startswith('scale='):
            try:
                scale = int(chunk.split('=', 1)[1])
            except ValueError:
                scale = None
        else:
            expr.append(chunk)

if not expr:
    sys.exit(0)

expression = expr[-1]
try:
    value = eval(expression, {"__builtins__": {}}, {})
except Exception:
    value = 0

if isinstance(value, bool):
    value = 1 if value else 0

if scale is None:
    print(value)
else:
    fmt = f"{{0:.{scale}f}}" if scale is not None and scale >= 0 else "{0}"
    print(fmt.format(value))
EOF
chmod +x "$BIN_DIR/bc"

cat >"$TMP_ROOT/browser.toml" <<'TOML'
proxy_servers = ["http://proxy.example"]
fingerprint_seed = "abc123"
user_agent = "Mozilla/5.0"
TOML

VVTV_ENV=(
  VVTV_BASE_DIR="$BASE"
  VVTV_DATA_DIR="$BASE/data"
  ARCHIVE_DIR="$BASE/storage/archive"
  QUEUE_DB="$BASE/data/queue.sqlite"
  CONFIG_FILE="$TMP_ROOT/browser.toml"
  CURATOR_LOG_DIR="$BASE/system/logs/curator"
)

VVTV_PATH_ENV=(
  PATH="$PATH"
)

(
  export "${VVTV_ENV[@]}" "${VVTV_PATH_ENV[@]}"
  "$ROOT_DIR/check_queue.sh" --recent 5 --json >/dev/null || true
)

(
  export "${VVTV_ENV[@]}" "${VVTV_PATH_ENV[@]}"
  "$ROOT_DIR/inject_emergency_loop.sh" --dry-run --count 1 >/dev/null || true
)

(
  export "${VVTV_ENV[@]}" "${VVTV_PATH_ENV[@]}" PROFILE_ROOT="$BASE/cache/browser_profiles" CONFIG_FILE="$TMP_ROOT/browser.toml"
  "$ROOT_DIR/browser_diagnose.sh" --json --profile profile-1 >/dev/null || true
)

(
  export "${VVTV_ENV[@]}" "${VVTV_PATH_ENV[@]}"
  "$ROOT_DIR/takedown.sh" --id sample-plan --dry-run >/dev/null || true
)

(
  export "${VVTV_ENV[@]}" "${VVTV_PATH_ENV[@]}"
  "$ROOT_DIR/restart_encoder.sh" --dry-run >/dev/null
)

(
  export "${VVTV_ENV[@]}" "${VVTV_PATH_ENV[@]}"
  "$ROOT_DIR/integrity_check.sh" --report "$BASE/system/reports/test_integrity.json" >/dev/null || true
)

(
  export "${VVTV_PATH_ENV[@]}" CLOUDFLARE_API_TOKEN=dummy CLOUDFLARE_ZONE_ID=zone CLOUDFLARE_RECORD_ID=rec CDN_LOG="$BASE/system/logs/cdn_failover.log"
  "$ROOT_DIR/switch_cdn.sh" cdn.backup.example --dry-run >/dev/null
)

echo "Incident playbook scripts linted and smoke-tested."
