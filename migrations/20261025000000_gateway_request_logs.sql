ALTER TABLE request_logs RENAME TO gateway_request_logs;

ALTER INDEX idx_request_logs_user_created RENAME TO idx_gateway_request_logs_user_created;
ALTER INDEX idx_request_logs_key_created RENAME TO idx_gateway_request_logs_key_created;

DO $$
DECLARE
    item RECORD;
BEGIN
    FOR item IN
        SELECT conname
        FROM pg_constraint
        WHERE conrelid = 'gateway_request_logs'::regclass AND conname LIKE 'request\_logs\_%'
    LOOP
        EXECUTE format(
            'ALTER TABLE gateway_request_logs RENAME CONSTRAINT %I TO %I',
            item.conname,
            'gateway_' || item.conname
        );
    END LOOP;
END $$;

ALTER TABLE gateway_request_logs
    ADD COLUMN request_id UUID,
    ADD COLUMN provider VARCHAR,
    ADD COLUMN error_category VARCHAR(50),
    ADD COLUMN streaming BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN latency_ms INTEGER CONSTRAINT gateway_request_logs_latency_check CHECK (latency_ms >= 0);

CREATE UNIQUE INDEX IF NOT EXISTS idx_gateway_request_logs_request_id ON gateway_request_logs(request_id) WHERE request_id IS NOT NULL;
