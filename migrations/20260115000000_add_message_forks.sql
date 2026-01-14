-- Migration: Add message forks support
-- Allows users to edit messages and explore alternative conversation paths

-- Add parent_id to create message tree structure
ALTER TABLE messages ADD COLUMN parent_id UUID REFERENCES messages(id) ON DELETE CASCADE;

-- Fork index within siblings (1-indexed: 1 = original, 2+ = alternatives)
ALTER TABLE messages ADD COLUMN fork_index INTEGER NOT NULL DEFAULT 1;

-- Flag to mark the currently active fork path
ALTER TABLE messages ADD COLUMN is_active_fork BOOLEAN NOT NULL DEFAULT TRUE;

-- Indexes for efficient tree traversal
CREATE INDEX IF NOT EXISTS idx_messages_parent ON messages(parent_id);
CREATE INDEX IF NOT EXISTS idx_messages_fork ON messages(parent_id, fork_index);

-- Note: Existing messages get parent_id = NULL, fork_index = 1, is_active_fork = TRUE
-- This maintains backward compatibility (treated as linear chain via created_at ordering)
