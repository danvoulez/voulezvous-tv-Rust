PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 30000000000;
PRAGMA busy_timeout = 5000;

BEGIN;

CREATE TABLE IF NOT EXISTS viewer_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL UNIQUE,
    viewer_id TEXT,
    join_time DATETIME NOT NULL,
    leave_time DATETIME NOT NULL,
    duration_seconds INTEGER NOT NULL,
    region TEXT NOT NULL,
    device TEXT NOT NULL,
    bandwidth_mbps REAL,
    engagement_score REAL,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_viewer_sessions_region ON viewer_sessions(region, join_time DESC);
CREATE INDEX IF NOT EXISTS idx_viewer_sessions_join ON viewer_sessions(join_time DESC);

COMMIT;
