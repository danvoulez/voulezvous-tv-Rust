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

CREATE TABLE IF NOT EXISTS replication_syncs (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    path TEXT,
    bytes_transferred INTEGER,
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_replication_syncs_ts ON replication_syncs(ts DESC);

CREATE TABLE IF NOT EXISTS replication_events (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    path TEXT,
    differences INTEGER,
    total_files INTEGER,
    drift_percent REAL,
    failover_triggered INTEGER
);

CREATE INDEX IF NOT EXISTS idx_replication_events_ts ON replication_events(ts DESC);

CREATE TABLE IF NOT EXISTS cdn_metrics (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    provider TEXT,
    cdn_hits INTEGER,
    latency_avg_ms REAL,
    cache_hit_rate REAL,
    origin_errors INTEGER
);

CREATE INDEX IF NOT EXISTS idx_cdn_metrics_ts ON cdn_metrics(ts DESC);

CREATE TABLE IF NOT EXISTS backup_syncs (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    provider TEXT,
    files_uploaded INTEGER,
    bytes_uploaded INTEGER,
    removed_segments TEXT,
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_backup_syncs_ts ON backup_syncs(ts DESC);

CREATE TABLE IF NOT EXISTS edge_latency (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    region TEXT,
    target TEXT,
    latency_ms REAL
);

CREATE INDEX IF NOT EXISTS idx_edge_latency_ts ON edge_latency(ts DESC);

CREATE TABLE IF NOT EXISTS cdn_tokens (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    path TEXT,
    token TEXT,
    expires_at DATETIME
);

CREATE INDEX IF NOT EXISTS idx_cdn_tokens_ts ON cdn_tokens(ts DESC);

COMMIT;
