CREATE TABLE shelves (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
        CHECK (
            length(name) BETWEEN 1 AND 80
            AND name = trim(name)
        ),
    normalized_name TEXT NOT NULL COLLATE NOCASE
        CHECK (
            length(normalized_name) BETWEEN 1 AND 80
            AND normalized_name = trim(normalized_name)
        ),
    position INTEGER NOT NULL
        CHECK (position >= 0),
    query_version INTEGER NOT NULL
        CHECK (query_version > 0),
    query_payload TEXT NOT NULL
        CHECK (
            length(query_payload) BETWEEN 2 AND 262144
            AND json_valid(query_payload)
            AND json_type(query_payload) = 'object'
        ),
    validity_state TEXT NOT NULL DEFAULT 'valid'
        CHECK (validity_state IN ('valid', 'needs_attention')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_shelves_normalized_name
    ON shelves (normalized_name);

CREATE UNIQUE INDEX idx_shelves_position
    ON shelves (position);

CREATE INDEX idx_shelves_validity_position
    ON shelves (validity_state, position, id);
