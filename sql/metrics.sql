PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 30000000000;
PRAGMA busy_timeout = 5000;

BEGIN;

CREATE TABLE IF NOT EXISTS metrics (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    buffer_duration_h REAL,
    queue_length INTEGER,
    played_last_hour INTEGER,
    failures_last_hour INTEGER,
    avg_cpu_load REAL,
    avg_temp_c REAL,
    latency_s REAL,
    stream_bitrate_mbps REAL,
    vmaf_live REAL
);

CREATE INDEX IF NOT EXISTS idx_metrics_ts ON metrics(ts DESC);

CREATE TABLE IF NOT EXISTS curator_failures (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    url TEXT,
    category TEXT,
    error_message TEXT,
    attempt INTEGER,
    proxy TEXT,
    remediation TEXT
);

CREATE INDEX IF NOT EXISTS idx_curator_failures_ts ON curator_failures(ts DESC);

CREATE TABLE IF NOT EXISTS curator_runs (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    scenario TEXT,
    url TEXT,
    success INTEGER,
    duration_ms INTEGER,
    screenshot_path TEXT,
    video_path TEXT,
    proxy_rotations INTEGER
);

CREATE INDEX IF NOT EXISTS idx_curator_runs_ts ON curator_runs(ts DESC);

CREATE TABLE IF NOT EXISTS proxy_rotations (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    exit_node TEXT
);

CREATE INDEX IF NOT EXISTS idx_proxy_rotations_ts ON proxy_rotations(ts DESC);

COMMIT;
