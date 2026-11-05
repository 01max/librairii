CREATE TABLE catalog_syncs (
    id TEXT PRIMARY KEY
        CHECK (
            length(id) = 36
            AND id = lower(id)
            AND id NOT GLOB '*[^0-9a-f-]*'
            AND substr(id, 9, 1) = '-'
            AND substr(id, 14, 1) = '-'
            AND substr(id, 19, 1) = '-'
            AND substr(id, 24, 1) = '-'
        ),
    locale TEXT NOT NULL
        CHECK (
            length(trim(locale)) > 0
            AND locale = trim(locale)
        ),
    status TEXT NOT NULL DEFAULT 'running'
        CHECK (status IN ('running', 'succeeded', 'failed', 'cancelled')),
    matched_story_count INTEGER NOT NULL DEFAULT 0
        CHECK (matched_story_count >= 0),
    error_code TEXT,
    error_message TEXT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    UNIQUE (id, locale),
    CHECK (
        (status = 'running' AND finished_at IS NULL)
        OR (status != 'running' AND finished_at IS NOT NULL)
    ),
    CHECK (
        status != 'failed'
        OR (
            length(trim(error_code)) > 0
            AND length(trim(error_message)) > 0
        )
    ),
    CHECK (
        status != 'succeeded'
        OR (error_code IS NULL AND error_message IS NULL)
    )
);

CREATE TABLE catalog_snapshots (
    id INTEGER PRIMARY KEY,
    sync_id TEXT NOT NULL,
    locale TEXT NOT NULL,
    raw_path TEXT NOT NULL
        CHECK (
            length(raw_path) > 0
            AND raw_path NOT LIKE '/%'
            AND raw_path NOT LIKE '../%'
            AND raw_path NOT LIKE '%/../%'
        ),
    raw_sha256 TEXT NOT NULL
        CHECK (
            length(raw_sha256) = 64
            AND raw_sha256 = lower(raw_sha256)
            AND raw_sha256 NOT GLOB '*[^0-9a-f]*'
        ),
    byte_size INTEGER NOT NULL
        CHECK (byte_size >= 0),
    record_count INTEGER NOT NULL
        CHECK (record_count >= 0),
    status TEXT NOT NULL DEFAULT 'staged'
        CHECK (status IN ('staged', 'active', 'superseded', 'rejected')),
    fetched_at TEXT NOT NULL,
    activated_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (sync_id, locale)
        REFERENCES catalog_syncs(id, locale) ON DELETE CASCADE,
    UNIQUE (sync_id),
    UNIQUE (id, locale),
    CHECK (
        (status IN ('staged', 'rejected') AND activated_at IS NULL)
        OR (status IN ('active', 'superseded') AND activated_at IS NOT NULL)
    )
);

CREATE TABLE official_story_metadata (
    id INTEGER PRIMARY KEY,
    snapshot_id INTEGER NOT NULL,
    story_uuid TEXT NOT NULL
        CHECK (
            length(story_uuid) = 36
            AND story_uuid = lower(story_uuid)
            AND story_uuid NOT GLOB '*[^0-9a-f-]*'
            AND substr(story_uuid, 9, 1) = '-'
            AND substr(story_uuid, 14, 1) = '-'
            AND substr(story_uuid, 19, 1) = '-'
            AND substr(story_uuid, 24, 1) = '-'
        ),
    locale TEXT NOT NULL
        CHECK (
            length(trim(locale)) > 0
            AND locale = trim(locale)
        ),
    title TEXT
        CHECK (title IS NULL OR length(trim(title)) > 0),
    description TEXT
        CHECK (description IS NULL OR length(trim(description)) > 0),
    author TEXT
        CHECK (author IS NULL OR length(trim(author)) > 0),
    publisher TEXT
        CHECK (publisher IS NULL OR length(trim(publisher)) > 0),
    language TEXT
        CHECK (language IS NULL OR length(trim(language)) > 0),
    duration_seconds INTEGER
        CHECK (duration_seconds IS NULL OR duration_seconds >= 0),
    minimum_age INTEGER
        CHECK (minimum_age IS NULL OR minimum_age >= 0),
    maximum_age INTEGER
        CHECK (maximum_age IS NULL OR maximum_age >= 0),
    artwork_id TEXT
        CHECK (artwork_id IS NULL OR length(trim(artwork_id)) > 0),
    provenance TEXT NOT NULL
        CHECK (provenance = 'lunii_catalog'),
    source_record_id TEXT
        CHECK (source_record_id IS NULL OR length(trim(source_record_id)) > 0),
    source_updated_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (snapshot_id, locale)
        REFERENCES catalog_snapshots(id, locale) ON DELETE CASCADE,
    UNIQUE (snapshot_id, story_uuid)
);

CREATE UNIQUE INDEX idx_catalog_snapshots_active_locale
    ON catalog_snapshots (locale)
    WHERE status = 'active';

CREATE INDEX idx_catalog_snapshots_locale_status
    ON catalog_snapshots (locale, status, fetched_at DESC, id DESC);

CREATE INDEX idx_catalog_syncs_status_started
    ON catalog_syncs (status, started_at DESC);

CREATE INDEX idx_official_metadata_uuid_locale
    ON official_story_metadata (story_uuid, locale, snapshot_id);

CREATE INDEX idx_official_metadata_snapshot_uuid
    ON official_story_metadata (snapshot_id, story_uuid);

CREATE TRIGGER catalog_syncs_validate_transition
BEFORE UPDATE OF status ON catalog_syncs
FOR EACH ROW
WHEN (
    NEW.status != OLD.status
    AND NOT (
        OLD.status = 'running'
        AND NEW.status IN ('succeeded', 'failed', 'cancelled')
    )
)
BEGIN
    SELECT RAISE(ABORT, 'invalid catalog sync status transition');
END;

CREATE TRIGGER catalog_snapshots_validate_insert
BEFORE INSERT ON catalog_snapshots
FOR EACH ROW
WHEN (
    SELECT status
    FROM catalog_syncs
    WHERE id = NEW.sync_id
) != 'running'
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshots require a running sync');
END;

CREATE TRIGGER catalog_snapshots_validate_transition
BEFORE UPDATE OF status ON catalog_snapshots
FOR EACH ROW
WHEN (
    NEW.status != OLD.status
    AND NOT (
        (OLD.status = 'staged' AND NEW.status IN ('active', 'rejected'))
        OR (OLD.status = 'active' AND NEW.status = 'superseded')
    )
)
BEGIN
    SELECT RAISE(ABORT, 'invalid catalog snapshot status transition');
END;

CREATE TRIGGER catalog_snapshots_preserve_content
BEFORE UPDATE OF sync_id, locale, raw_path, raw_sha256, byte_size, record_count, fetched_at
ON catalog_snapshots
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshot content is immutable');
END;

CREATE TRIGGER official_metadata_require_staged_insert
BEFORE INSERT ON official_story_metadata
FOR EACH ROW
WHEN (
    SELECT status
    FROM catalog_snapshots
    WHERE id = NEW.snapshot_id
) != 'staged'
BEGIN
    SELECT RAISE(ABORT, 'official metadata requires a staged snapshot');
END;

CREATE TRIGGER official_metadata_require_staged_update
BEFORE UPDATE ON official_story_metadata
FOR EACH ROW
WHEN (
    SELECT status
    FROM catalog_snapshots
    WHERE id = OLD.snapshot_id
) != 'staged'
BEGIN
    SELECT RAISE(ABORT, 'active official metadata is immutable');
END;
