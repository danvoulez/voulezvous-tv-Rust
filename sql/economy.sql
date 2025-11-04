PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 30000000000;
PRAGMA busy_timeout = 5000;

BEGIN;

CREATE TABLE IF NOT EXISTS economy_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    event_type TEXT NOT NULL,
    value_eur REAL NOT NULL,
    source TEXT NOT NULL,
    context TEXT NOT NULL,
    proof TEXT NOT NULL,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_economy_type_ts ON economy_events(event_type, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_economy_context ON economy_events(context);

CREATE TABLE IF NOT EXISTS micro_spots (
    id TEXT PRIMARY KEY,
    sponsor TEXT NOT NULL,
    visual_style TEXT NOT NULL,
    duration_s INTEGER NOT NULL,
    value_eur REAL NOT NULL,
    cadence_min_minutes INTEGER NOT NULL,
    cadence_max_minutes INTEGER NOT NULL,
    asset_path TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    next_available_at DATETIME,
    expires_at DATETIME,
    last_injected_at DATETIME,
    total_injections INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS micro_spot_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    contract_id TEXT NOT NULL,
    injected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    queue_entry_id INTEGER,
    FOREIGN KEY(contract_id) REFERENCES micro_spots(id)
);

CREATE INDEX IF NOT EXISTS idx_micro_spots_active ON micro_spots(active, next_available_at);
CREATE INDEX IF NOT EXISTS idx_micro_spots_expires ON micro_spots(expires_at);

COMMIT;
