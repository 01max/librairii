CREATE TABLE tag_definitions (
    id INTEGER PRIMARY KEY,
    key TEXT NOT NULL
        CHECK (length(trim(key)) > 0),
    normalized_key TEXT NOT NULL UNIQUE COLLATE NOCASE
        CHECK (
            length(normalized_key) > 0
            AND normalized_key = trim(normalized_key)
        ),
    label TEXT NOT NULL
        CHECK (length(trim(label)) > 0),
    color TEXT NOT NULL
        CHECK (
            length(color) = 7
            AND substr(color, 1, 1) = '#'
            AND substr(color, 2) NOT GLOB '*[^0-9A-Fa-f]*'
        ),
    kind TEXT NOT NULL
        CHECK (kind IN ('boolean', 'choice')),
    source TEXT NOT NULL
        CHECK (source IN ('user', 'builtin', 'derived')),
    presentation TEXT NOT NULL DEFAULT 'default'
        CHECK (presentation IN ('default', 'warning', 'system')),
    position INTEGER NOT NULL
        CHECK (position >= 0),
    is_protected INTEGER NOT NULL DEFAULT 0
        CHECK (is_protected IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (source, position),
    CHECK (
        (source = 'user' AND is_protected = 0)
        OR (source IN ('builtin', 'derived') AND is_protected = 1)
    )
);

CREATE TABLE tag_values (
    id INTEGER PRIMARY KEY,
    definition_id INTEGER NOT NULL
        REFERENCES tag_definitions(id) ON DELETE CASCADE,
    key TEXT NOT NULL
        CHECK (length(trim(key)) > 0),
    normalized_key TEXT NOT NULL COLLATE NOCASE
        CHECK (
            length(normalized_key) > 0
            AND normalized_key = trim(normalized_key)
        ),
    label TEXT NOT NULL
        CHECK (length(trim(label)) > 0),
    position INTEGER NOT NULL
        CHECK (position >= 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (definition_id, normalized_key),
    UNIQUE (definition_id, position),
    UNIQUE (id, definition_id)
);

CREATE TABLE story_tag_assignments (
    id INTEGER PRIMARY KEY,
    story_id INTEGER NOT NULL
        REFERENCES stories(id) ON DELETE CASCADE,
    definition_id INTEGER NOT NULL
        REFERENCES tag_definitions(id) ON DELETE CASCADE,
    value_id INTEGER,
    source TEXT NOT NULL
        CHECK (source IN ('manual', 'derived')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (value_id, definition_id)
        REFERENCES tag_values(id, definition_id) ON DELETE CASCADE
);

CREATE TRIGGER tag_values_require_choice_insert
BEFORE INSERT ON tag_values
FOR EACH ROW
WHEN (
    SELECT kind
    FROM tag_definitions
    WHERE id = NEW.definition_id
) != 'choice'
BEGIN
    SELECT RAISE(ABORT, 'tag values require a choice definition');
END;

CREATE TRIGGER tag_values_require_choice_update
BEFORE UPDATE OF definition_id ON tag_values
FOR EACH ROW
WHEN (
    SELECT kind
    FROM tag_definitions
    WHERE id = NEW.definition_id
) != 'choice'
BEGIN
    SELECT RAISE(ABORT, 'tag values require a choice definition');
END;

CREATE TRIGGER tag_assignments_validate_insert
BEFORE INSERT ON story_tag_assignments
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN (
            SELECT kind
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) = 'boolean' AND NEW.value_id IS NOT NULL
        THEN RAISE(ABORT, 'boolean tag assignments cannot have a value')
        WHEN (
            SELECT kind
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) = 'choice' AND NEW.value_id IS NULL
        THEN RAISE(ABORT, 'choice tag assignments require a value')
        WHEN NEW.source = 'manual' AND (
            SELECT source
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) = 'derived'
        THEN RAISE(ABORT, 'derived tags cannot be assigned manually')
        WHEN NEW.source = 'derived' AND (
            SELECT source
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) != 'derived'
        THEN RAISE(ABORT, 'derived assignments require a derived tag')
    END;
END;

CREATE TRIGGER tag_assignments_validate_update
BEFORE UPDATE OF definition_id, value_id, source ON story_tag_assignments
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN (
            SELECT kind
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) = 'boolean' AND NEW.value_id IS NOT NULL
        THEN RAISE(ABORT, 'boolean tag assignments cannot have a value')
        WHEN (
            SELECT kind
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) = 'choice' AND NEW.value_id IS NULL
        THEN RAISE(ABORT, 'choice tag assignments require a value')
        WHEN NEW.source = 'manual' AND (
            SELECT source
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) = 'derived'
        THEN RAISE(ABORT, 'derived tags cannot be assigned manually')
        WHEN NEW.source = 'derived' AND (
            SELECT source
            FROM tag_definitions
            WHERE id = NEW.definition_id
        ) != 'derived'
        THEN RAISE(ABORT, 'derived assignments require a derived tag')
    END;
END;

CREATE TRIGGER tag_definitions_preserve_kind_integrity
BEFORE UPDATE OF kind ON tag_definitions
FOR EACH ROW
WHEN (
    NEW.kind != OLD.kind
    AND (
        EXISTS (
            SELECT 1
            FROM tag_values
            WHERE definition_id = OLD.id
        )
        OR EXISTS (
            SELECT 1
            FROM story_tag_assignments
            WHERE definition_id = OLD.id
        )
    )
)
BEGIN
    SELECT RAISE(ABORT, 'assigned tag definition kind cannot change');
END;

CREATE TRIGGER tag_definitions_preserve_source_integrity
BEFORE UPDATE OF source ON tag_definitions
FOR EACH ROW
WHEN (
    NEW.source != OLD.source
    AND EXISTS (
        SELECT 1
        FROM story_tag_assignments
        WHERE definition_id = OLD.id
    )
)
BEGIN
    SELECT RAISE(ABORT, 'assigned tag definition source cannot change');
END;

CREATE UNIQUE INDEX idx_story_tag_boolean_assignment
    ON story_tag_assignments (story_id, definition_id)
    WHERE value_id IS NULL;

CREATE UNIQUE INDEX idx_story_tag_choice_assignment
    ON story_tag_assignments (story_id, definition_id, value_id)
    WHERE value_id IS NOT NULL;

CREATE INDEX idx_tag_definitions_source_position
    ON tag_definitions (source, position, id);

CREATE INDEX idx_tag_values_definition_position
    ON tag_values (definition_id, position, id);

CREATE INDEX idx_story_tag_assignments_story
    ON story_tag_assignments (story_id, definition_id, value_id);

CREATE INDEX idx_story_tag_assignments_filter
    ON story_tag_assignments (definition_id, value_id, story_id);

CREATE INDEX idx_story_tag_assignments_source
    ON story_tag_assignments (source, definition_id);
