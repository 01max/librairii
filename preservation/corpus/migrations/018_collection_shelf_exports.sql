-- Keep the constrained legacy discriminator intact instead of rewriting history.
DROP TRIGGER file_operations_require_export_scope;
DROP TRIGGER file_operations_export_scope_immutable;
ALTER TABLE file_operations RENAME COLUMN export_source_type TO legacy_export_source_type;
ALTER TABLE file_operations ADD COLUMN export_source_type TEXT CHECK(
    export_source_type IS NULL OR export_source_type IN
    ('selection', 'current_query', 'shelf', 'shelves', 'collection', 'collections', 'curated_shelf', 'curated_shelves')
);
UPDATE file_operations SET export_source_type = legacy_export_source_type;

ALTER TABLE file_operation_items ADD COLUMN export_skip_code TEXT
    CHECK(export_skip_code IS NULL OR export_skip_code IN ('story_marked_broken', 'story_unavailable'));
DROP TRIGGER file_operation_items_require_export_scope;
DROP TRIGGER file_operation_items_export_scope_immutable;

CREATE TRIGGER file_operations_require_export_scope
BEFORE INSERT ON file_operations
FOR EACH ROW
WHEN NEW.kind = 'export' AND (
    NEW.export_source_type IS NULL
    OR NEW.export_source_shelf_ids IS NULL
    OR json_type(NEW.export_source_shelf_ids) != 'array'
    OR NEW.export_source_shelf_names IS NULL
    OR json_type(NEW.export_source_shelf_names) != 'array'
    OR json_array_length(NEW.export_source_shelf_ids)
        != json_array_length(NEW.export_source_shelf_names)
    OR (
        NEW.export_source_type IN ('selection', 'current_query')
        AND json_array_length(NEW.export_source_shelf_ids) != 0
    )
    OR (
        NEW.export_source_type IN ('shelf', 'collection', 'curated_shelf')
        AND json_array_length(NEW.export_source_shelf_ids) != 1
    )
    OR (
        NEW.export_source_type IN ('shelves', 'collections', 'curated_shelves')
        AND json_array_length(NEW.export_source_shelf_ids) < 2
    )
    OR NEW.export_destination IS NULL
    OR length(trim(NEW.export_destination)) = 0
)
BEGIN
    SELECT RAISE(ABORT, 'export operation scope is incomplete');
END;

CREATE TRIGGER file_operations_export_scope_immutable
BEFORE UPDATE OF
    kind,
    legacy_export_source_type,
    export_source_type,
    export_source_shelf_ids,
    export_source_shelf_names,
    export_destination,
    total_items,
    total_bytes
ON file_operations
FOR EACH ROW
WHEN OLD.kind = 'export' AND (
    OLD.kind IS NOT NEW.kind
    OR OLD.legacy_export_source_type IS NOT NEW.legacy_export_source_type
    OR OLD.export_source_type IS NOT NEW.export_source_type
    OR OLD.export_source_shelf_ids IS NOT NEW.export_source_shelf_ids
    OR OLD.export_source_shelf_names IS NOT NEW.export_source_shelf_names
    OR OLD.export_destination IS NOT NEW.export_destination
    OR OLD.total_items IS NOT NEW.total_items
    OR OLD.total_bytes IS NOT NEW.total_bytes
)
BEGIN
    SELECT RAISE(ABORT, 'export operation scope is immutable');
END;

CREATE TRIGGER file_operation_items_require_export_scope
BEFORE INSERT ON file_operation_items
FOR EACH ROW
WHEN (
    SELECT kind
    FROM file_operations
    WHERE id = NEW.operation_id
) = 'export' AND (
    NEW.story_uuid IS NULL
    OR length(trim(NEW.story_uuid)) = 0
    OR NEW.story_title IS NULL
    OR length(trim(NEW.story_title)) = 0
    OR (NEW.export_skip_code IS NOT NULL AND NEW.total_bytes != 0)
    OR (NEW.export_skip_code IS NULL AND (
    NEW.story_id IS NULL
    OR NEW.resolved_story_id IS NULL
    OR NEW.story_uuid IS NULL
    OR length(trim(NEW.story_uuid)) = 0
    OR NEW.story_title IS NULL
    OR length(trim(NEW.story_title)) = 0
    OR NEW.output_name IS NULL
    OR length(trim(NEW.output_name)) = 0
    OR NEW.archive_relative_path IS NULL
    OR length(trim(NEW.archive_relative_path)) = 0
    OR NEW.archive_sha256 IS NULL
    ))
)
BEGIN
    SELECT RAISE(ABORT, 'export operation item scope is incomplete');
END;

CREATE TRIGGER file_operation_items_export_scope_immutable
BEFORE UPDATE OF
    export_skip_code,
    resolved_story_id,
    source_name,
    total_bytes,
    story_uuid,
    story_title,
    output_name,
    archive_relative_path,
    archive_sha256
ON file_operation_items
FOR EACH ROW
WHEN (
    SELECT kind
    FROM file_operations
    WHERE id = OLD.operation_id
) = 'export' AND (
    OLD.export_skip_code IS NOT NEW.export_skip_code
    OR OLD.resolved_story_id IS NOT NEW.resolved_story_id
    OR OLD.source_name IS NOT NEW.source_name
    OR OLD.total_bytes IS NOT NEW.total_bytes
    OR OLD.story_uuid IS NOT NEW.story_uuid
    OR OLD.story_title IS NOT NEW.story_title
    OR OLD.output_name IS NOT NEW.output_name
    OR OLD.archive_relative_path IS NOT NEW.archive_relative_path
    OR OLD.archive_sha256 IS NOT NEW.archive_sha256
)
BEGIN
    SELECT RAISE(ABORT, 'export operation item scope is immutable');
END;
