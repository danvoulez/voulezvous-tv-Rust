BEGIN;

CREATE TABLE IF NOT EXISTS economy_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    event_type TEXT NOT NULL,
    value_eur REAL NOT NULL,
    proof TEXT,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_economy_type ON economy_events(event_type, ts DESC);

COMMIT;
