-- Add structured content parts for messages (text + image references)

ALTER TABLE messages
    ADD COLUMN IF NOT EXISTS content_parts JSONB;

CREATE INDEX IF NOT EXISTS idx_messages_content_parts_gin ON messages USING GIN (content_parts);
