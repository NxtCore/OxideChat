ALTER TABLE gateway_projects ADD COLUMN IF NOT EXISTS max_concurrent_requests INTEGER CONSTRAINT gateway_projects_max_concurrent_requests_check CHECK (max_concurrent_requests > 0);
ALTER TABLE gateway_api_keys ADD COLUMN IF NOT EXISTS max_concurrent_requests INTEGER CONSTRAINT gateway_api_keys_max_concurrent_requests_check CHECK (max_concurrent_requests > 0);

CREATE TABLE IF NOT EXISTS gateway_rate_limit_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES gateway_projects(id) ON DELETE CASCADE,
    api_key_id UUID REFERENCES gateway_api_keys(id) ON DELETE CASCADE,
    route_class VARCHAR NOT NULL CONSTRAINT gateway_rate_limit_rules_route_class_check CHECK (route_class IN ('inference', 'utility')),
    metric VARCHAR NOT NULL CONSTRAINT gateway_rate_limit_rules_metric_check CHECK (metric IN ('requests', 'tokens')),
    capacity BIGINT NOT NULL CONSTRAINT gateway_rate_limit_rules_capacity_check CHECK (capacity > 0),
    refill_period_seconds INTEGER NOT NULL CONSTRAINT gateway_rate_limit_rules_refill_period_seconds_check CHECK (refill_period_seconds > 0),
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT gateway_rate_limit_rules_scope_check CHECK ((project_id IS NULL) <> (api_key_id IS NULL)),
    CONSTRAINT gateway_utility_requests_only CHECK (route_class <> 'utility' OR metric = 'requests')
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_gateway_rate_limit_rules_project_window ON gateway_rate_limit_rules (project_id, route_class, metric, refill_period_seconds) WHERE project_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_gateway_rate_limit_rules_key_window ON gateway_rate_limit_rules (api_key_id, route_class, metric, refill_period_seconds) WHERE api_key_id IS NOT NULL;

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'gateway.errors.limit_requests', 'Request limit exceeded. Please retry later.'),
    ('en', 'gateway.errors.limit_tokens', 'Token limit exceeded. Reduce the requested tokens or retry later.'),
    ('en', 'gateway.errors.limit_concurrency', 'Concurrent request limit exceeded. Please retry later.'),
    ('en', 'gateway.errors.limiter_unavailable', 'Request admission is temporarily unavailable.'),
    ('en', 'gateway.errors.limiter_overloaded', 'Too many requests are waiting for admission. Please retry shortly.'),
    ('en', 'gateway.errors.limiter_misconfigured', 'The rate limit configuration is invalid. Please contact support.'),
    ('de', 'gateway.errors.limit_requests', 'Anfragelimit überschritten. Bitte später erneut versuchen.'),
    ('de', 'gateway.errors.limit_tokens', 'Tokenlimit überschritten. Bitte weniger Tokens anfordern oder später erneut versuchen.'),
    ('de', 'gateway.errors.limit_concurrency', 'Limit gleichzeitiger Anfragen überschritten. Bitte später erneut versuchen.'),
    ('de', 'gateway.errors.limiter_unavailable', 'Die Anfrageprüfung ist vorübergehend nicht verfügbar.'),
    ('de', 'gateway.errors.limiter_overloaded', 'Zu viele Anfragen warten auf Zulassung. Bitte in Kürze erneut versuchen.'),
    ('de', 'gateway.errors.limiter_misconfigured', 'Die Ratenlimit-Konfiguration ist ungültig. Bitte den Support kontaktieren.')
ON CONFLICT (language, key_path) DO NOTHING;
