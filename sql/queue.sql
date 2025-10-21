BEGIN;

CREATE TABLE IF NOT EXISTS playout_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id TEXT NOT NULL,
    asset_path TEXT NOT NULL,
    duration_s INTEGER,
    status TEXT DEFAULT 'queued',
    curation_score REAL,
    priority INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME,
    node_origin TEXT,
    play_started_at DATETIME,
    play_finished_at DATETIME,
    failure_reason TEXT,
    content_kind TEXT,
    FOREIGN KEY(plan_id) REFERENCES plans(plan_id)
);

CREATE INDEX IF NOT EXISTS idx_queue_status ON playout_queue(status, created_at);
CREATE INDEX IF NOT EXISTS idx_queue_priority ON playout_queue(priority DESC, created_at ASC);

CREATE TRIGGER IF NOT EXISTS trg_queue_touch
AFTER UPDATE ON playout_queue
FOR EACH ROW
WHEN NEW.updated_at IS OLD.updated_at
BEGIN
    UPDATE playout_queue
    SET updated_at = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;

COMMIT;
