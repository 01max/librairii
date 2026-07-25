ALTER TABLE official_story_metadata
    ADD COLUMN title_normalized TEXT NOT NULL DEFAULT '';

DROP TRIGGER official_metadata_require_staged_update;

CREATE TRIGGER official_metadata_require_staged_update
BEFORE UPDATE OF
    snapshot_id,
    story_uuid,
    locale,
    title,
    description,
    author,
    publisher,
    language,
    duration_seconds,
    minimum_age,
    maximum_age,
    artwork_id,
    provenance,
    source_record_id,
    source_updated_at,
    created_at
ON official_story_metadata
FOR EACH ROW
WHEN (
    SELECT status
    FROM catalog_snapshots
    WHERE id = OLD.snapshot_id
) != 'staged'
BEGIN
    SELECT RAISE(ABORT, 'active official metadata is immutable');
END;

CREATE INDEX idx_official_metadata_language_story
    ON official_story_metadata (language, story_uuid, snapshot_id);
