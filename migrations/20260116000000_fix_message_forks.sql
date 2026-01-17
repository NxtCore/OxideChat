-- Migration: Fix existing message fork data
-- For chats created before fork support was fully implemented, 
-- messages have parent_id = NULL and all have is_active_fork = TRUE.
-- This migration fixes the structure for simple linear conversations.

-- Step 1: For each chat with all parent_id = NULL, set parent_id to create a linked chain
-- This processes messages in chronological order, linking each to the previous one
DO $$
DECLARE
    prev_id UUID := NULL;
    outer_rec RECORD;
    inner_rec RECORD;
BEGIN
    FOR outer_rec IN 
        SELECT DISTINCT chat_id FROM messages 
        WHERE parent_id IS NULL
        ORDER BY chat_id
    LOOP
        prev_id := NULL;
        FOR inner_rec IN 
            SELECT id FROM messages 
            WHERE chat_id = outer_rec.chat_id 
            ORDER BY created_at ASC
        LOOP
            IF prev_id IS NOT NULL THEN
                UPDATE messages SET parent_id = prev_id WHERE id = inner_rec.id;
            END IF;
            prev_id := inner_rec.id;
        END LOOP;
    END LOOP;
END $$;

-- Step 2: Ensure all messages in the main chain have is_active_fork = TRUE
-- and fork_index = 1 (since they are the original messages)
UPDATE messages SET is_active_fork = TRUE WHERE fork_index = 1;
