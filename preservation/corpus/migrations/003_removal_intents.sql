CREATE TABLE removal_intents (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    story_id INTEGER NOT NULL UNIQUE REFERENCES stories(id) ON DELETE CASCADE,
    managed_path TEXT NOT NULL
        CHECK (
            length(managed_path) > 0
            AND managed_path NOT LIKE '/%'
            AND managed_path NOT LIKE '../%'
            AND managed_path NOT LIKE '%/../%'
        ),
    trash_path TEXT NOT NULL UNIQUE
        CHECK (
            length(trash_path) > 0
            AND trash_path NOT LIKE '/%'
            AND trash_path NOT LIKE '../%'
            AND trash_path NOT LIKE '%/../%'
        ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_removal_intents_story ON removal_intents(story_id);
