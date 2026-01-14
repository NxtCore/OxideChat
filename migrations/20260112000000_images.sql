-- Images table for CDN storage
-- Stores images uploaded from tools (imagegen, etc.) for serving via URL

CREATE TABLE IF NOT EXISTS images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    data BYTEA,  -- NULL if using file storage
    file_path VARCHAR(500),  -- Path relative to storage root (for file storage)
    mime_type VARCHAR(64) NOT NULL DEFAULT 'image/png',
	size_bytes BIGINT NOT NULL,
    source VARCHAR(50),  -- 'imagegen', 'upload', etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_images_created_at ON images(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_images_user_id ON images(user_id);
