-- Add branch tracking columns to chats table
ALTER TABLE chats ADD COLUMN branched_from_chat_id UUID REFERENCES chats(id) ON DELETE SET NULL;
ALTER TABLE chats ADD COLUMN branched_from_message_id UUID REFERENCES messages(id) ON DELETE SET NULL;

-- Index for efficient lookup of branched chats
CREATE INDEX idx_chats_branched_from ON chats(branched_from_chat_id) WHERE branched_from_chat_id IS NOT NULL;
