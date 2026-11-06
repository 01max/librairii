ALTER TABLE stories
ADD COLUMN display_name_normalized TEXT NOT NULL DEFAULT '';

UPDATE stories
SET display_name_normalized = lower(
    COALESCE(
        NULLIF(trim(embedded_title), ''),
        'Story ' || uuid
    )
);

CREATE INDEX idx_stories_display_name_normalized
    ON stories (display_name_normalized, id);
