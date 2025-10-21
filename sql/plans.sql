PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 30000000000;
PRAGMA busy_timeout = 5000;

BEGIN;

CREATE TABLE IF NOT EXISTS plans (
    plan_id TEXT PRIMARY KEY,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    kind TEXT NOT NULL,
    title TEXT,
    source_url TEXT,
    duration_est_s INTEGER,
    resolution_observed TEXT,
    curation_score REAL DEFAULT 0.5,
    status TEXT DEFAULT 'planned',
    license_proof TEXT,
    hd_missing INTEGER DEFAULT 0,
    node_origin TEXT,
    updated_at DATETIME,
    failure_count INTEGER DEFAULT 0,
    tags TEXT,
    trending_score REAL DEFAULT 0.0,
    desire_vector TEXT,
    engagement_score REAL DEFAULT 0.0
);

CREATE INDEX IF NOT EXISTS idx_plans_status ON plans(status);
CREATE INDEX IF NOT EXISTS idx_plans_score ON plans(curation_score DESC);
CREATE INDEX IF NOT EXISTS idx_plans_updated ON plans(updated_at);
CREATE INDEX IF NOT EXISTS idx_plans_kind ON plans(kind, status);

CREATE TRIGGER IF NOT EXISTS trg_plans_touch
AFTER UPDATE ON plans
FOR EACH ROW
WHEN NEW.updated_at IS OLD.updated_at
BEGIN
    UPDATE plans
    SET updated_at = CURRENT_TIMESTAMP
    WHERE plan_id = NEW.plan_id;
END;

CREATE TABLE IF NOT EXISTS plan_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id TEXT NOT NULL,
    status_from TEXT,
    status_to TEXT,
    note TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(plan_id) REFERENCES plans(plan_id)
);

CREATE INDEX IF NOT EXISTS idx_plan_attempts_plan ON plan_attempts(plan_id, created_at DESC);

CREATE TABLE IF NOT EXISTS plan_blacklist (
    domain TEXT PRIMARY KEY,
    reason TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS plan_metrics (
    metric TEXT PRIMARY KEY,
    value REAL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

COMMIT;
