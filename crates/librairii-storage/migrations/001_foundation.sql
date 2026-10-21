CREATE TABLE librairii_schema_identity (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    product TEXT NOT NULL CHECK (product = 'librairii'),
    schema_family TEXT NOT NULL CHECK (schema_family = 'wails-go-sqlite'),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO librairii_schema_identity (singleton, product, schema_family)
VALUES (1, 'librairii', 'wails-go-sqlite');
