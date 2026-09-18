ALTER TABLE gateway_projects ADD COLUMN max_concurrent_requests INTEGER CHECK (max_concurrent_requests > 0);
ALTER TABLE gateway_api_keys ADD COLUMN max_concurrent_requests INTEGER CHECK (max_concurrent_requests > 0);

CREATE TABLE gateway_rate_limit_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES gateway_projects(id) ON DELETE CASCADE,
    api_key_id UUID REFERENCES gateway_api_keys(id) ON DELETE CASCADE,
    route_class VARCHAR NOT NULL CHECK (route_class IN ('inference', 'utility')),
    metric VARCHAR NOT NULL CHECK (metric IN ('requests', 'tokens')),
    capacity BIGINT NOT NULL CHECK (capacity > 0),
    refill_period_seconds INTEGER NOT NULL CHECK (refill_period_seconds > 0),
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((project_id IS NULL) <> (api_key_id IS NULL))
);

CREATE UNIQUE INDEX gateway_project_rate_window ON gateway_rate_limit_rules (project_id, route_class, metric, refill_period_seconds) WHERE project_id IS NOT NULL;
CREATE UNIQUE INDEX gateway_key_rate_window ON gateway_rate_limit_rules (api_key_id, route_class, metric, refill_period_seconds) WHERE api_key_id IS NOT NULL;

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'gateway.errors.limit_requests', 'Request limit exceeded. Please retry later.'),
    ('en', 'gateway.errors.limit_tokens', 'Token limit exceeded. Reduce the requested tokens or retry later.'),
    ('en', 'gateway.errors.limit_concurrency', 'Concurrent request limit exceeded. Please retry later.'),
    ('en', 'gateway.errors.limiter_unavailable', 'Request admission is temporarily unavailable.'),
    ('de', 'gateway.errors.limit_requests', 'Anfragelimit überschritten. Bitte später erneut versuchen.'),
    ('de', 'gateway.errors.limit_tokens', 'Tokenlimit überschritten. Bitte weniger Tokens anfordern oder später erneut versuchen.'),
    ('de', 'gateway.errors.limit_concurrency', 'Limit gleichzeitiger Anfragen überschritten. Bitte später erneut versuchen.'),
    ('de', 'gateway.errors.limiter_unavailable', 'Die Anfrageprüfung ist vorübergehend nicht verfügbar.')
ON CONFLICT (language, key_path) DO NOTHING;
