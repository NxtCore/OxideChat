CREATE TABLE gateway_projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    team_id UUID REFERENCES teams(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX gateway_projects_owner_idx ON gateway_projects(owner_id);
CREATE INDEX gateway_projects_team_idx ON gateway_projects(team_id) WHERE team_id IS NOT NULL;

CREATE TABLE gateway_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES gateway_projects(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    secret_hash TEXT NOT NULL,
    key_prefix VARCHAR(48) NOT NULL,
    last_four VARCHAR(4) NOT NULL,
    scopes JSONB NOT NULL DEFAULT '["inference:read", "inference:write"]'::jsonb,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX gateway_api_keys_prefix_idx ON gateway_api_keys(key_prefix);
CREATE INDEX gateway_api_keys_project_idx ON gateway_api_keys(project_id);
