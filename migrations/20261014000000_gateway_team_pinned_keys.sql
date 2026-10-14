ALTER TABLE gateway_api_keys ADD COLUMN IF NOT EXISTS user_id UUID REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE gateway_api_keys ADD COLUMN IF NOT EXISTS team_id UUID REFERENCES teams(id) ON DELETE CASCADE;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'gateway_api_keys' AND column_name = 'project_id') THEN
        UPDATE gateway_api_keys k
        SET user_id = p.owner_id,
            team_id = COALESCE(
                p.team_id,
                (
                    SELECT t.id
                    FROM teams t
                    JOIN team_members tm ON tm.team_id = t.id AND tm.user_id = p.owner_id
                    WHERE t.is_default
                )
            ),
            is_enabled = k.is_enabled AND p.is_enabled
        FROM gateway_projects p
        WHERE p.id = k.project_id;

        DELETE FROM gateway_rate_limit_rules WHERE project_id IS NOT NULL;
    END IF;
END $$;

DELETE FROM gateway_api_keys WHERE team_id IS NULL;
ALTER TABLE gateway_api_keys ALTER COLUMN team_id SET NOT NULL;
ALTER TABLE gateway_api_keys DROP COLUMN IF EXISTS project_id;

CREATE INDEX IF NOT EXISTS gateway_api_keys_team_idx ON gateway_api_keys(team_id);
CREATE INDEX IF NOT EXISTS gateway_api_keys_user_idx ON gateway_api_keys(user_id);

ALTER TABLE gateway_rate_limit_rules DROP CONSTRAINT IF EXISTS gateway_rate_limit_rules_scope_check;
DROP INDEX IF EXISTS idx_gateway_rate_limit_rules_project_window;
ALTER TABLE gateway_rate_limit_rules DROP COLUMN IF EXISTS project_id;
ALTER TABLE gateway_rate_limit_rules ADD COLUMN IF NOT EXISTS team_id UUID REFERENCES teams(id) ON DELETE CASCADE;
ALTER TABLE gateway_rate_limit_rules ADD CONSTRAINT gateway_rate_limit_rules_scope_check CHECK ((team_id IS NULL) <> (api_key_id IS NULL));
CREATE UNIQUE INDEX IF NOT EXISTS idx_gateway_rate_limit_rules_team_window ON gateway_rate_limit_rules (team_id, route_class, metric, refill_period_seconds) WHERE team_id IS NOT NULL;

ALTER TABLE teams ADD COLUMN IF NOT EXISTS gateway_max_concurrent_requests INTEGER CONSTRAINT teams_gateway_max_concurrent_requests_check CHECK (gateway_max_concurrent_requests > 0);

DROP TABLE IF EXISTS gateway_projects;
