CREATE TABLE stories (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE
        CHECK (
            length(uuid) = 36
            AND substr(uuid, 9, 1) = '-'
            AND substr(uuid, 14, 1) = '-'
            AND substr(uuid, 19, 1) = '-'
            AND substr(uuid, 24, 1) = '-'
        ),
    embedded_title TEXT,
    embedded_description TEXT,
    embedded_artwork_path TEXT
        CHECK (
            embedded_artwork_path IS NULL
            OR (
                embedded_artwork_path NOT LIKE '/%'
                AND embedded_artwork_path NOT LIKE '../%'
                AND embedded_artwork_path NOT LIKE '%/../%'
            )
        ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE story_archives (
    id INTEGER PRIMARY KEY,
    story_id INTEGER NOT NULL UNIQUE REFERENCES stories(id) ON DELETE CASCADE,
    original_filename TEXT NOT NULL CHECK (length(trim(original_filename)) > 0),
    detected_format TEXT NOT NULL
        CHECK (detected_format IN (
            'plain_pk',
            'v1_pk',
            'v2_pk',
            'generic_pk',
            'zip',
            'seven_zip',
            'studio_zip'
        )),
    sha256 TEXT NOT NULL UNIQUE
        CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
    managed_path TEXT NOT NULL UNIQUE
        CHECK (
            length(managed_path) > 0
            AND managed_path NOT LIKE '/%'
            AND managed_path NOT LIKE '../%'
            AND managed_path NOT LIKE '%/../%'
        ),
    validation_state TEXT NOT NULL DEFAULT 'valid'
        CHECK (validation_state IN ('valid', 'missing', 'invalid')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_stories_uuid ON stories(uuid);
CREATE INDEX idx_story_archives_sha256 ON story_archives(sha256);
CREATE INDEX idx_story_archives_validation_state ON story_archives(validation_state);

CREATE TABLE file_operations (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    kind TEXT NOT NULL CHECK (kind IN ('import', 'metadata_sync', 'export')),
    status TEXT NOT NULL
        CHECK (status IN (
            'queued',
            'running',
            'succeeded',
            'partially_succeeded',
            'failed',
            'cancelled',
            'interrupted'
        )),
    completed_items INTEGER NOT NULL DEFAULT 0 CHECK (completed_items >= 0),
    total_items INTEGER NOT NULL DEFAULT 0 CHECK (total_items >= 0),
    cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancel_requested IN (0, 1)),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT,
    finished_at TEXT,
    CHECK (completed_items <= total_items)
);

CREATE TABLE file_operation_items (
    id INTEGER PRIMARY KEY,
    operation_id TEXT NOT NULL REFERENCES file_operations(id) ON DELETE CASCADE,
    story_id INTEGER REFERENCES stories(id) ON DELETE SET NULL,
    source_name TEXT NOT NULL,
    status TEXT NOT NULL
        CHECK (status IN (
            'pending',
            'running',
            'succeeded',
            'skipped',
            'conflicted',
            'failed',
            'cancelled'
        )),
    outcome_code TEXT,
    outcome_message TEXT,
    completed_bytes INTEGER NOT NULL DEFAULT 0 CHECK (completed_bytes >= 0),
    total_bytes INTEGER NOT NULL DEFAULT 0 CHECK (total_bytes >= 0),
    CHECK (completed_bytes <= total_bytes)
);

CREATE INDEX idx_file_operations_status ON file_operations(status, created_at);
CREATE INDEX idx_file_operation_items_operation ON file_operation_items(operation_id, id);
CREATE INDEX idx_file_operation_items_story ON file_operation_items(story_id);
