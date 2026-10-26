ALTER TABLE team_members
    ADD COLUMN IF NOT EXISTS role VARCHAR NOT NULL DEFAULT 'member'
    CONSTRAINT team_members_role_check CHECK (role IN ('member', 'admin'));

CREATE INDEX IF NOT EXISTS idx_team_members_team_role ON team_members(team_id) WHERE role = 'admin';

DO $$
DECLARE
    item RECORD;
BEGIN
    FOR item IN
        SELECT c.conrelid::regclass::text AS table_name, c.conname
        FROM pg_constraint c
        JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY (c.conkey)
        WHERE c.contype = 'f'
          AND a.attname = 'user_id'
          AND c.conrelid IN ('usage_events'::regclass, 'gateway_request_logs'::regclass)
    LOOP
        EXECUTE format('ALTER TABLE %s DROP CONSTRAINT %I', item.table_name, item.conname);
    END LOOP;
END $$;

ALTER TABLE usage_events ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE usage_events ADD CONSTRAINT usage_events_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE gateway_request_logs ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE gateway_request_logs ADD CONSTRAINT gateway_request_logs_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;
