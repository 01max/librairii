use librairii_storage::{
    IsolatedLibrary, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
};
use librairii_testkit::TestRoots;
use std::{error::Error, fs, path::Path};
type TestResult = Result<(), Box<dyn Error>>;
fn library(roots: &TestRoots) -> Result<IsolatedLibrary, Box<dyn Error>> {
    Ok(IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "A0000000-0000-4000-8000-000000000001".parse()?,
    )?)
}
fn fixture(prefix: u32) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../preservation/corpus/databases/prefix-{prefix:03}.sqlite3"
    ))
}

#[test]
fn foreign_probe_preserves_main_wal_shm_and_explicit_recovery_never_replaces() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let database = library.layout().database();
    let files = [
        database.clone(),
        database.with_file_name("librairii.sqlite3-wal"),
        database.with_file_name("librairii.sqlite3-shm"),
    ];
    let bytes = [b"foreign\0\xff".as_slice(), b"exactWAL", b"exactSHM"];
    for (path, bytes) in files.iter().zip(bytes) {
        fs::write(path, bytes)?;
    }
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::ForeignSchema)
    ));
    for (path, bytes) in files.iter().zip(bytes) {
        assert_eq!(fs::read(path)?, bytes);
    }
    #[cfg(unix)]
    {
        let receipt = library.preserve_foreign_schema()?;
        for (path, bytes) in files.iter().zip(bytes) {
            assert!(!path.exists());
            assert_eq!(
                fs::read(
                    receipt
                        .directory()
                        .join(path.file_name().ok_or("missing filename")?)
                )?,
                bytes
            );
        }
        assert!(!database.exists());
        assert!(matches!(
            library.preserve_foreign_schema(),
            Err(StorageError::NoSchemaConflict)
        ));
    }
    #[cfg(not(unix))]
    {
        assert!(matches!(
            library.preserve_foreign_schema(),
            Err(StorageError::DirectoryDurabilityUnavailable)
        ));
        for (path, bytes) in files.iter().zip(bytes) {
            assert_eq!(fs::read(path)?, bytes);
        }
    }
    Ok(())
}

#[test]
fn future_and_gapped_ledgers_are_not_relocated_or_replaced() -> TestResult {
    for future in [false, true] {
        let roots = TestRoots::new()?;
        let library = library(&roots)?;
        fs::copy(fixture(18), library.layout().database())?;
        let connection = rusqlite::Connection::open(library.layout().database())?;
        if future {
            connection.execute("INSERT INTO schema_migrations(version) VALUES(19)", [])?;
        } else {
            connection.execute("DELETE FROM schema_migrations WHERE version=7", [])?;
        }
        drop(connection);
        let before = fs::read(library.layout().database())?;
        assert!(matches!(
            Database::open_isolated(&library),
            Err(StorageError::MigrationConflict)
        ));
        assert!(matches!(
            library.preserve_foreign_schema(),
            Err(StorageError::MigrationConflict)
        ));
        assert_eq!(fs::read(library.layout().database())?, before);
    }
    Ok(())
}

#[test]
fn cancelled_backup_preserves_source_and_never_claims_a_snapshot() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    let token = WorkCancellation::default();
    token.cancel();
    assert!(matches!(
        database.backup(token)?.wait(),
        Err(StorageError::Cancelled)
    ));
    assert_eq!(
        fs::read_dir(
            library
                .layout()
                .database()
                .parent()
                .ok_or("missing database directory")?
        )?
        .count(),
        3
    ); // main+live WAL/SHM, no backup
    database.close()?;
    Ok(())
}

#[test]
fn online_backup_includes_uncheckpointed_committed_wal_and_is_standalone() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    let outcome = database
        .transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE librairii_schema_identity SET created_at='committed-WAL-only'",
                [],
            )?;
            Ok(())
        })?
        .wait()?;
    assert!(matches!(outcome, TransactionOutcome::Committed(())));
    #[cfg(unix)]
    {
        let backup = database.backup(WorkCancellation::default())?.wait()?;
        assert_eq!(backup.prefix().version(), 18);
        assert_eq!(backup.counts().get("schema_migrations"), Some(&18));
        let read = rusqlite::Connection::open_with_flags(
            backup.path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        assert_eq!(
            read.query_row(
                "SELECT created_at FROM librairii_schema_identity",
                [],
                |row| row.get::<_, String>(0)
            )?,
            "committed-WAL-only"
        );
        assert_eq!(
            read.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))?,
            "ok"
        );
        assert!(
            !backup
                .path()
                .with_file_name(format!(
                    "{}-wal",
                    backup
                        .path()
                        .file_name()
                        .ok_or("missing backup name")?
                        .to_str()
                        .ok_or("invalid backup name")?
                ))
                .exists()
        );
        drop(read);
    }
    #[cfg(not(unix))]
    assert!(matches!(
        database.backup(WorkCancellation::default())?.wait(),
        Err(StorageError::DirectoryDurabilityUnavailable)
    ));
    database.close()?;
    Ok(())
}

#[test]
fn compatible_prefix_upgrades_keep_verified_backup_and_archive_bytes() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    fs::copy(fixture(1), library.layout().database())?;
    let archive = library.layout().archives().join("untouched.pk");
    fs::write(&archive, b"original archive bytes\0\xff")?;
    #[cfg(unix)]
    {
        let mut database = Database::open_isolated(&library)?;
        database.close()?;
        let backups = fs::read_dir(
            library
                .layout()
                .database()
                .parent()
                .ok_or("missing parent")?,
        )?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("librairii.pre-migration-v002-"))
        })
        .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        let read = rusqlite::Connection::open_with_flags(
            backups[0].path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        assert_eq!(
            read.query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                .get::<_, i64>(0))?,
            1
        );
        drop(read);
        let mut reopened = Database::open_isolated(&library)?;
        reopened.close()?;
        assert_eq!(
            fs::read_dir(
                library
                    .layout()
                    .database()
                    .parent()
                    .ok_or("missing parent")?
            )?
            .count(),
            2
        ); // main + one retained backup
    }
    #[cfg(not(unix))]
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::DirectoryDurabilityUnavailable)
    ));
    assert_eq!(fs::read(archive)?, b"original archive bytes\0\xff");
    Ok(())
}

#[test]
fn foreign_key_corruption_is_preserved_and_never_upgraded_or_relocated() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    fs::copy(fixture(18), library.layout().database())?;
    let connection = rusqlite::Connection::open(library.layout().database())?;
    connection.pragma_update(None, "foreign_keys", false)?;
    connection.execute("UPDATE story_archives SET story_id=424242", [])?;
    drop(connection);
    let before = fs::read(library.layout().database())?;
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::IntegrityFailed)
    ));
    assert!(matches!(
        library.preserve_foreign_schema(),
        Err(StorageError::IntegrityFailed)
    ));
    assert_eq!(fs::read(library.layout().database())?, before);
    Ok(())
}

#[cfg(unix)]
#[test]
fn aliased_or_symlinked_database_is_not_a_qualified_owned_copy() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let external = tempfile::tempdir()?;
    let source = external.path().join("independent-source.sqlite3");
    fs::copy(fixture(18), &source)?;
    let before = fs::read(&source)?;
    fs::hard_link(&source, library.layout().database())?;
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::PathEscape)
    ));
    assert_eq!(fs::read(&source)?, before);
    fs::remove_file(library.layout().database())?;
    std::os::unix::fs::symlink(&source, library.layout().database())?;
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::PathEscape)
    ));
    assert_eq!(fs::read(source)?, before);
    Ok(())
}
