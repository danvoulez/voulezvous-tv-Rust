-- VVTV Public API - Tenant Management Schema
-- Multi-tenant authentication, quotas, and usage tracking

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

-- Tenants table
CREATE TABLE IF NOT EXISTS tenants (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active', 'suspended', 'pending')) DEFAULT 'active',
    plan TEXT NOT NULL CHECK(plan IN ('free', 'pro', 'enterprise')) DEFAULT 'free',
    
    -- Authentication
    api_public_key TEXT UNIQUE NOT NULL,
    api_secret_hash TEXT NOT NULL,  -- argon2id hash
    
    -- Rate limits
    rpm_limit INTEGER NOT NULL DEFAULT 60,      -- requests per minute
    rps_burst INTEGER NOT NULL DEFAULT 10,      -- burst capacity
    tpm_limit INTEGER DEFAULT NULL,             -- tokens per minute (optional)
    
    -- Quotas
    monthly_request_quota INTEGER DEFAULT NULL,  -- monthly request limit
    monthly_token_quota INTEGER DEFAULT NULL,    -- monthly token limit
    
    -- Permissions
    scopes TEXT NOT NULL DEFAULT '["infer"]',   -- JSON array of allowed scopes
    
    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT,
    notes TEXT
);

-- Nonce tracking (prevent replay attacks)
CREATE TABLE IF NOT EXISTS nonces (
    tenant_id TEXT NOT NULL,
    nonce TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, nonce),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

-- Usage events (detailed request tracking)
CREATE TABLE IF NOT EXISTS usage_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    tenant_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    
    -- Request details
    route TEXT NOT NULL,
    method TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    
    -- LLM specifics
    task TEXT,
    strategy TEXT,
    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,
    provider TEXT,
    from_cache BOOLEAN DEFAULT FALSE,
    
    -- Network
    ip_address TEXT,
    user_agent TEXT,
    
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

-- Daily usage rollups (for efficient quota checking)
CREATE TABLE IF NOT EXISTS usage_rollups (
    tenant_id TEXT NOT NULL,
    date TEXT NOT NULL,  -- YYYY-MM-DD format
    
    -- Aggregated metrics
    total_requests INTEGER DEFAULT 0,
    total_tokens_in INTEGER DEFAULT 0,
    total_tokens_out INTEGER DEFAULT 0,
    total_duration_ms INTEGER DEFAULT 0,
    
    -- Status code breakdown
    status_2xx INTEGER DEFAULT 0,
    status_4xx INTEGER DEFAULT 0,
    status_5xx INTEGER DEFAULT 0,
    
    -- Performance metrics
    avg_duration_ms REAL DEFAULT 0,
    p95_duration_ms REAL DEFAULT 0,
    
    -- Cache metrics
    cache_hits INTEGER DEFAULT 0,
    cache_misses INTEGER DEFAULT 0,
    
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    PRIMARY KEY (tenant_id, date),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

-- API keys rotation history
CREATE TABLE IF NOT EXISTS key_rotations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tenant_id TEXT NOT NULL,
    old_public_key TEXT NOT NULL,
    new_public_key TEXT NOT NULL,
    rotated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    rotated_by TEXT,
    reason TEXT,
    
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

-- Rate limit violations
CREATE TABLE IF NOT EXISTS rate_limit_violations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    tenant_id TEXT NOT NULL,
    ip_address TEXT,
    violation_type TEXT NOT NULL,  -- 'rpm', 'rps', 'quota'
    current_value INTEGER NOT NULL,
    limit_value INTEGER NOT NULL,
    
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_tenants_api_key ON tenants(api_public_key);
CREATE INDEX IF NOT EXISTS idx_tenants_status ON tenants(status);
CREATE INDEX IF NOT EXISTS idx_nonces_timestamp ON nonces(timestamp);
CREATE INDEX IF NOT EXISTS idx_usage_events_tenant_timestamp ON usage_events(tenant_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_usage_events_request_id ON usage_events(request_id);
CREATE INDEX IF NOT EXISTS idx_usage_rollups_tenant_date ON usage_rollups(tenant_id, date);
CREATE INDEX IF NOT EXISTS idx_rate_violations_tenant_timestamp ON rate_limit_violations(tenant_id, timestamp);

-- Cleanup trigger for old nonces (keep only last 24 hours)
CREATE TRIGGER IF NOT EXISTS cleanup_old_nonces
    AFTER INSERT ON nonces
    WHEN NEW.timestamp > 0
BEGIN
    DELETE FROM nonces 
    WHERE timestamp < (strftime('%s', 'now') - 86400);
END;

-- Update trigger for tenants
CREATE TRIGGER IF NOT EXISTS update_tenants_timestamp
    AFTER UPDATE ON tenants
BEGIN
    UPDATE tenants SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Sample data for testing (remove in production)
INSERT OR IGNORE INTO tenants (
    id, name, plan, api_public_key, api_secret_hash, rpm_limit, rps_burst, scopes
) VALUES (
    'test_tenant_001',
    'Test Tenant',
    'pro',
    'vvtv_test_key_001',
    '$argon2id$v=19$m=65536,t=3,p=4$placeholder_hash',  -- Replace with real hash
    120,
    20,
    '["infer", "usage"]'
);

-- Views for common queries
CREATE VIEW IF NOT EXISTS tenant_usage_summary AS
SELECT 
    t.id,
    t.name,
    t.plan,
    t.status,
    COALESCE(SUM(ur.total_requests), 0) as monthly_requests,
    COALESCE(SUM(ur.total_tokens_in + ur.total_tokens_out), 0) as monthly_tokens,
    COALESCE(AVG(ur.avg_duration_ms), 0) as avg_response_time_ms,
    t.rpm_limit,
    t.monthly_request_quota,
    t.monthly_token_quota
FROM tenants t
LEFT JOIN usage_rollups ur ON t.id = ur.tenant_id 
    AND ur.date >= date('now', 'start of month')
WHERE t.status = 'active'
GROUP BY t.id, t.name, t.plan, t.status, t.rpm_limit, t.monthly_request_quota, t.monthly_token_quota;

-- Function to check quota usage (would be implemented in application)
-- This is a placeholder for the logic that would be in Rust code