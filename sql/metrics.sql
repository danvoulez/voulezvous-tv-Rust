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

COMMIT;
