ALTER TABLE stories ADD COLUMN is_broken INTEGER NOT NULL DEFAULT 0
    CHECK (is_broken IN (0, 1));

-- Retain the identity after deletion so old saved queries and URLs can be read
-- even if a later user tag reuses the definition's numeric ID.
CREATE TABLE legacy_broken_tag (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    definition_id INTEGER NOT NULL CHECK (definition_id > 0)
);

INSERT INTO legacy_broken_tag (singleton, definition_id)
SELECT 1, id FROM tag_definitions
WHERE normalized_key = 'broken' AND source = 'builtin';

UPDATE stories SET is_broken = 1
WHERE id IN (
    SELECT story_id FROM story_tag_assignments
    WHERE definition_id IN (SELECT definition_id FROM legacy_broken_tag)
      AND value_id IS NULL
);

DROP TRIGGER protected_broken_requires_canonical_insert;
DROP TRIGGER protected_broken_rejects_update;
DROP TRIGGER protected_broken_rejects_delete;

DELETE FROM tag_definitions
WHERE id IN (SELECT definition_id FROM legacy_broken_tag);
