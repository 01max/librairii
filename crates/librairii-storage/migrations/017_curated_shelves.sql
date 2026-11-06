CREATE TABLE shelves (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 80 AND name = trim(name)),
    normalized_name TEXT NOT NULL COLLATE NOCASE UNIQUE
        CHECK(length(normalized_name) BETWEEN 1 AND 80 AND normalized_name = trim(normalized_name)),
    position INTEGER NOT NULL UNIQUE CHECK(position >= 0),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE shelf_stories (
    shelf_id INTEGER NOT NULL REFERENCES shelves(id) ON DELETE CASCADE,
    story_uuid TEXT NOT NULL CHECK(length(story_uuid) = 36),
    position INTEGER NOT NULL CHECK(position >= 0),
    protected INTEGER NOT NULL DEFAULT 1 CHECK(protected IN (0, 1)),
    cached_title TEXT NOT NULL DEFAULT '',
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(shelf_id, story_uuid),
    UNIQUE(shelf_id, position)
);

CREATE TABLE shelf_sources (
    id INTEGER PRIMARY KEY,
    shelf_id INTEGER NOT NULL REFERENCES shelves(id) ON DELETE CASCADE,
    collection_id INTEGER REFERENCES collections(id),
    collection_name TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(id, shelf_id),
    CHECK(active = 0 OR collection_id IS NOT NULL)
);
CREATE UNIQUE INDEX idx_shelf_active_sources
    ON shelf_sources(shelf_id, collection_id) WHERE active = 1;
CREATE INDEX idx_collection_source_shelves ON shelf_sources(collection_id, active, shelf_id);

CREATE TABLE shelf_story_origins (
    shelf_id INTEGER NOT NULL,
    story_uuid TEXT NOT NULL,
    source_id INTEGER NOT NULL,
    accepted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(shelf_id, story_uuid, source_id),
    FOREIGN KEY(shelf_id, story_uuid) REFERENCES shelf_stories(shelf_id, story_uuid) ON DELETE CASCADE,
    FOREIGN KEY(source_id, shelf_id) REFERENCES shelf_sources(id, shelf_id)
);

CREATE TABLE shelf_exclusions (
    shelf_id INTEGER NOT NULL REFERENCES shelves(id) ON DELETE CASCADE,
    story_uuid TEXT NOT NULL CHECK(length(story_uuid) = 36),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(shelf_id, story_uuid)
);
