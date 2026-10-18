CREATE TABLE IF NOT EXISTS request_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    team_id UUID REFERENCES teams(id) ON DELETE SET NULL,
    api_key_id UUID REFERENCES gateway_api_keys(id) ON DELETE SET NULL,
    model_id UUID REFERENCES models(id) ON DELETE SET NULL,
    public_model_id VARCHAR NOT NULL,
    request_type VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL CONSTRAINT request_logs_status_check CHECK (status IN ('success', 'error')),
    status_code INTEGER,
    input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    reasoning_tokens INTEGER NOT NULL DEFAULT 0 CHECK (reasoning_tokens >= 0),
    cost_total NUMERIC(12,6) NOT NULL DEFAULT 0 CHECK (cost_total >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_request_logs_user_created ON request_logs(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_request_logs_key_created ON request_logs(api_key_id, created_at);

CREATE TABLE IF NOT EXISTS request_log_retention (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    retention_days INTEGER NOT NULL DEFAULT 30
        CONSTRAINT request_log_retention_days_check CHECK (retention_days BETWEEN 1 AND 3650),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
