-- Initial schema for OxideChat
-- This migration creates the core tables for the chat application

-- Translations table - stores all i18n translations
CREATE TABLE IF NOT EXISTS i18n_translations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    language VARCHAR(10) NOT NULL,
    key_path VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    is_override BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(language, key_path)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_i18n_translations_language ON i18n_translations(language);