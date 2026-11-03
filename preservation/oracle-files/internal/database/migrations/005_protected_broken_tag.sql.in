CREATE TRIGGER protected_broken_requires_canonical_insert
BEFORE INSERT ON tag_definitions
FOR EACH ROW
WHEN (
    lower(NEW.normalized_key) = 'broken'
    AND NOT (
        NEW.key COLLATE BINARY = 'broken'
        AND NEW.normalized_key COLLATE BINARY = 'broken'
        AND NEW.label COLLATE BINARY = 'Broken'
        AND NEW.color COLLATE BINARY = '#FF705C'
        AND NEW.kind = 'boolean'
        AND NEW.source = 'builtin'
        AND NEW.presentation = 'warning'
        AND NEW.position = 0
        AND NEW.is_protected = 1
    )
)
BEGIN
    SELECT RAISE(ABORT, 'broken tag identity is protected');
END;

CREATE TRIGGER protected_broken_rejects_update
BEFORE UPDATE ON tag_definitions
FOR EACH ROW
WHEN (
    lower(OLD.normalized_key) = 'broken'
    OR lower(NEW.normalized_key) = 'broken'
)
BEGIN
    SELECT RAISE(ABORT, 'broken tag identity is protected');
END;

CREATE TRIGGER protected_broken_rejects_delete
BEFORE DELETE ON tag_definitions
FOR EACH ROW
WHEN lower(OLD.normalized_key) = 'broken'
BEGIN
    SELECT RAISE(ABORT, 'broken tag identity is protected');
END;
