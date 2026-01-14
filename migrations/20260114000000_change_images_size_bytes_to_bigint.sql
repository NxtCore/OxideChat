-- Change images.size_bytes from INTEGER to BIGINT for safety on 64-bit systems

BEGIN;

ALTER TABLE images
    ALTER COLUMN size_bytes TYPE BIGINT USING size_bytes::BIGINT;

COMMIT;
