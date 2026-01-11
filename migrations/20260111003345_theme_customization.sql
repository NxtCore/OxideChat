-- Add theme customization support
-- User-specific theme preferences and global config

-- Add theme fields to user_preferences
ALTER TABLE user_preferences
ADD COLUMN IF NOT EXISTS theme_css_vars JSONB DEFAULT '{}'::jsonb,
ADD COLUMN IF NOT EXISTS custom_theme_urls JSONB DEFAULT '[]'::jsonb;

-- Global configuration table for instance-wide settings
CREATE TABLE IF NOT EXISTS global_config (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert default theme config if not exists
INSERT INTO global_config (key, value)
VALUES ('default_theme', '{}'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Index for faster lookups
CREATE INDEX IF NOT EXISTS idx_global_config_key ON global_config(key);
