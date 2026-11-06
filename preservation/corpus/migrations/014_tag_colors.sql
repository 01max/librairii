DROP TRIGGER protected_broken_rejects_update;
DROP TRIGGER protected_broken_requires_canonical_insert;

CREATE TRIGGER protected_broken_requires_canonical_insert
BEFORE INSERT ON tag_definitions
FOR EACH ROW
WHEN lower(NEW.normalized_key) = 'broken' AND NOT (
    NEW.key COLLATE BINARY = 'broken'
    AND NEW.normalized_key COLLATE BINARY = 'broken'
    AND NEW.label COLLATE BINARY = 'Broken'
    AND NEW.kind = 'boolean' AND NEW.source = 'builtin'
    AND NEW.presentation = 'warning' AND NEW.position = 0
    AND NEW.is_protected = 1
)
BEGIN
    SELECT RAISE(ABORT, 'broken tag identity is protected');
END;

CREATE TRIGGER protected_broken_rejects_update
BEFORE UPDATE ON tag_definitions
FOR EACH ROW
WHEN (lower(OLD.normalized_key) = 'broken' OR lower(NEW.normalized_key) = 'broken')
AND (
    NEW.id != OLD.id OR NEW.key COLLATE BINARY != OLD.key COLLATE BINARY
    OR NEW.normalized_key COLLATE BINARY != OLD.normalized_key COLLATE BINARY
    OR NEW.label COLLATE BINARY != OLD.label COLLATE BINARY
    OR NEW.kind != OLD.kind OR NEW.source != OLD.source
    OR NEW.presentation != OLD.presentation OR NEW.position != OLD.position
    OR NEW.is_protected != OLD.is_protected OR NEW.created_at != OLD.created_at
)
BEGIN
    SELECT RAISE(ABORT, 'broken tag identity is protected');
END;

UPDATE tag_definitions SET color = '#267D5D'
WHERE normalized_key = 'age' AND source = 'derived' AND color = '#FF705C';
