ALTER TABLE teams ADD COLUMN IF NOT EXISTS gateway_enabled BOOLEAN NOT NULL DEFAULT TRUE;

CREATE INDEX IF NOT EXISTS gateway_api_keys_expires_idx ON gateway_api_keys(expires_at) WHERE revoked_at IS NULL AND expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS gateway_api_keys_last_used_idx ON gateway_api_keys(last_used_at);

INSERT INTO permissions (name, description) VALUES
    ('admin.developer.*', 'All developer gateway administration permissions (wildcard)'),
    ('admin.developer.view', 'View all gateway API keys, team gateway settings, request metadata and gateway events'),
    ('admin.developer.keys.edit', 'Disable, enable or revoke any gateway API key'),
    ('admin.developer.limits.edit', 'Manage team gateway rate limits and concurrency caps'),
    ('admin.developer.access.edit', 'Turn gateway access on or off for a whole team'),
    ('admin.developer.audit.view', 'View the gateway audit trail')
ON CONFLICT (name) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'admin' AND p.name LIKE 'admin.developer.%'
ON CONFLICT DO NOTHING;
