use librairii_storage::{
    StorageError,
    migrations::{MIGRATIONS, SCHEMA_FAMILY, SCHEMA_PRODUCT, SchemaPrefix},
};
use rusqlite::{Connection, OpenFlags};
use std::{error::Error, fs, path::Path};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn every_embedded_sql_byte_matches_the_admitted_corpus_and_go_source() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let entries = fs::read_dir(root.join("preservation/corpus/migrations"))?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(entries.len(), MIGRATIONS.len());
    for migration in MIGRATIONS {
        let corpus = fs::read(
            root.join("preservation/corpus/migrations")
                .join(migration.filename),
        )?;
        let oracle = fs::read(
            root.join("internal/database/migrations")
                .join(migration.filename),
        )?;
        assert_eq!(migration.sql.as_bytes(), corpus);
        assert_eq!(corpus, oracle);
    }
    Ok(())
}

#[test]
fn every_actual_prefix_has_the_unchanged_identity_and_contiguous_ledger() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/databases");
    for version in 1..=18 {
        let path = root.join(format!("prefix-{version:03}.sqlite3"));
        let before = fs::read(&path)?;
        // All admitted inputs are closed standalone snapshots, with no WAL.
        // immutable avoids creating SHM/WAL on these frozen read-only inputs.
        let uri = format!("file:{}?immutable=1", path.display());
        let mut connection = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        let (product, family): (String, String) = connection.query_row(
            "SELECT product,schema_family FROM librairii_schema_identity WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let versions = connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")?
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let prefix = SchemaPrefix::recognize(&product, &family, &versions)?;
        assert_eq!(prefix.version(), version);
        assert_eq!(SchemaPrefix::inspect(&mut connection)?, prefix);
        assert_eq!(prefix.pending().count(), (18 - version) as usize);
        drop(connection);
        assert_eq!(fs::read(&path)?, before);
        assert!(
            !path
                .with_file_name(format!("prefix-{version:03}.sqlite3-wal"))
                .exists()
        );
        assert!(
            !path
                .with_file_name(format!("prefix-{version:03}.sqlite3-shm"))
                .exists()
        );
    }
    Ok(())
}

#[test]
fn forged_ledger_or_missing_trigger_is_rejected_without_schema_repair() -> TestResult {
    let mut connection = Connection::open_in_memory()?;
    connection.execute_batch(librairii_storage::migrations::LEDGER_SQL)?;
    for migration in MIGRATIONS {
        connection.execute_batch(migration.sql)?;
        connection.execute(
            "INSERT INTO schema_migrations(version) VALUES(?)",
            [migration.version],
        )?;
    }
    assert_eq!(SchemaPrefix::inspect(&mut connection)?.version(), 18);
    let trigger: String = connection.query_row(
        "SELECT name FROM sqlite_schema WHERE type='trigger' ORDER BY name LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    connection.execute_batch(&format!(
        "DROP TRIGGER \"{}\"",
        trigger.replace('"', "\"\"")
    ))?;
    assert!(matches!(
        SchemaPrefix::inspect(&mut connection),
        Err(StorageError::MigrationConflict)
    ));
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type='trigger' AND name=?",
        [&trigger],
        |row| row.get(0),
    )?;
    assert_eq!(count, 0);
    connection.execute("DELETE FROM schema_migrations WHERE version=7", [])?;
    assert!(matches!(
        SchemaPrefix::inspect(&mut connection),
        Err(StorageError::MigrationConflict)
    ));
    Ok(())
}

#[test]
fn foreign_empty_gapped_duplicate_unsorted_and_future_ledgers_fail_closed() {
    for (product, family) in [
        ("other", SCHEMA_FAMILY),
        (SCHEMA_PRODUCT, "rust-sqlite"),
        ("", ""),
    ] {
        assert!(matches!(
            SchemaPrefix::recognize(product, family, &[1]),
            Err(StorageError::ForeignSchema)
        ));
    }
    for versions in [
        vec![],
        vec![0],
        vec![2],
        vec![1, 3],
        vec![1, 1],
        vec![2, 1],
        (1..=19).collect(),
    ] {
        assert!(matches!(
            SchemaPrefix::recognize(SCHEMA_PRODUCT, SCHEMA_FAMILY, &versions),
            Err(StorageError::MigrationConflict)
        ));
    }
}
