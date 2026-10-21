ALTER TABLE shelves RENAME TO collections;
DROP INDEX idx_shelves_normalized_name;
DROP INDEX idx_shelves_position;
DROP INDEX idx_shelves_validity_position;
CREATE UNIQUE INDEX idx_collections_normalized_name ON collections(normalized_name);
CREATE UNIQUE INDEX idx_collections_position ON collections(position);
CREATE INDEX idx_collections_validity_position ON collections(validity_state, position, id);
ALTER TABLE collections ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0);

CREATE TRIGGER collections_definition_revision
AFTER UPDATE OF name, query_version, query_payload, validity_state ON collections
WHEN OLD.name IS NOT NEW.name OR OLD.query_version IS NOT NEW.query_version
  OR OLD.query_payload IS NOT NEW.query_payload OR OLD.validity_state IS NOT NEW.validity_state
BEGIN
    UPDATE collections SET revision = OLD.revision + 1 WHERE id = NEW.id;
END;
