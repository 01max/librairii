CREATE TABLE catalog_artworks (
    id TEXT PRIMARY KEY
        CHECK (
            length(id) = 64
            AND id = lower(id)
            AND id NOT GLOB '*[^0-9a-f]*'
        ),
    source_url TEXT NOT NULL UNIQUE
        CHECK (
            length(trim(source_url)) > 0
            AND source_url = trim(source_url)
        ),
    managed_path TEXT UNIQUE
        CHECK (
            managed_path IS NULL
            OR (
                length(managed_path) > 0
                AND managed_path NOT LIKE '/%'
                AND managed_path NOT LIKE '../%'
                AND managed_path NOT LIKE '%/../%'
            )
        ),
    content_type TEXT
        CHECK (
            content_type IS NULL
            OR content_type IN ('image/png', 'image/jpeg', 'image/webp')
        ),
    sha256 TEXT
        CHECK (
            sha256 IS NULL
            OR (
                length(sha256) = 64
                AND sha256 = lower(sha256)
                AND sha256 NOT GLOB '*[^0-9a-f]*'
            )
        ),
    byte_size INTEGER
        CHECK (byte_size IS NULL OR byte_size > 0),
    etag TEXT,
    last_modified TEXT,
    cached_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (
            managed_path IS NULL
            AND content_type IS NULL
            AND sha256 IS NULL
            AND byte_size IS NULL
            AND cached_at IS NULL
        )
        OR (
            managed_path IS NOT NULL
            AND content_type IS NOT NULL
            AND sha256 IS NOT NULL
            AND byte_size IS NOT NULL
            AND cached_at IS NOT NULL
        )
    )
);

CREATE INDEX idx_catalog_artworks_cached
    ON catalog_artworks (cached_at, id);

CREATE TRIGGER official_metadata_require_registered_artwork
BEFORE INSERT ON official_story_metadata
FOR EACH ROW
WHEN (
    NEW.artwork_id IS NOT NULL
    AND NOT EXISTS (
        SELECT 1
        FROM catalog_artworks
        WHERE id = NEW.artwork_id
    )
)
BEGIN
    SELECT RAISE(ABORT, 'official metadata artwork is not registered');
END;

CREATE TRIGGER catalog_artworks_preserve_identity
BEFORE UPDATE OF id, source_url ON catalog_artworks
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'catalog artwork identity is immutable');
END;
