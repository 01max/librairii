//! Verified standalone online snapshots and conservative owned-root upgrades.
//!
//! Copy probes touch only an independent private workspace; they do not confer
//! adoption authority. Existing opening is reachable only through a retained
//! isolated-root runtime. Production adoption/OS exclusion is a separate gate.
use crate::{
    Result, StorageError,
    database::{BUSY_TIMEOUT, WorkCancellation},
    migrations::{Migration, SchemaPrefix},
};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug)]
pub struct BackupReceipt {
    path: PathBuf,
    prefix: SchemaPrefix,
    counts: BTreeMap<String, i64>,
}
impl BackupReceipt {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub const fn prefix(&self) -> SchemaPrefix {
        self.prefix
    }
    pub fn counts(&self) -> &BTreeMap<String, i64> {
        &self.counts
    }
}

#[derive(Debug)]
pub struct PreservedSchema {
    directory: PathBuf,
}
impl PreservedSchema {
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}
impl crate::IsolatedLibrary {
    /// Explicit owned-root action; normal open never relocates foreign files or
    /// initializes a replacement. A partial move fences this authority until
    /// recovery/restart, retaining its precise recovery directory in the error.
    pub fn preserve_foreign_schema(&self) -> Result<PreservedSchema> {
        self.claim_runtime()?;
        let mut may_have_moved = false;
        let result = (|| {
            let database = self
                .layout()
                .resolve(&crate::ManagedPath::parse("db/librairii.sqlite3")?)?;
            let staging = self
                .layout()
                .resolve(&crate::ManagedPath::parse("staging")?)?;
            if path_stamp(&database)?.is_none() {
                return Err(StorageError::NoSchemaConflict);
            }
            match probe_owned(&database, &staging) {
                Err(StorageError::ForeignSchema) => (),
                Ok(_) => return Err(StorageError::NoSchemaConflict),
                Err(error) => return Err(error),
            }
            require_directory_durability()?;
            let parent = database.parent().ok_or(StorageError::InvalidManagedPath)?;
            let directory = tempfile::Builder::new()
                .prefix("schema-conflict-recovery-")
                .tempdir_in(parent)
                .map_err(|e| StorageError::io("reserve foreign schema recovery", e))?
                .keep();
            may_have_moved = true;
            let moved = (|| {
                preserve_files(&database, &directory)?;
                sync_directory(&directory)?;
                sync_directory(parent)
            })();
            if let Err(cause) = moved {
                return Err(StorageError::PreservationFailed {
                    directory,
                    cause: Box::new(cause),
                });
            }
            Ok(PreservedSchema { directory })
        })();
        if result.is_ok() || !may_have_moved {
            self.release_runtime();
        }
        result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Stamp {
    length: u64,
    modified: SystemTime,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    inode: u64,
}
fn stamp(metadata: &fs::Metadata) -> Result<Stamp> {
    if !metadata.is_file() {
        return Err(StorageError::PathEscape);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(StorageError::PathEscape);
        }
    }
    Ok(Stamp {
        length: metadata.len(),
        modified: metadata
            .modified()
            .map_err(|e| StorageError::io("read database modification time", e))?,
        #[cfg(unix)]
        dev: {
            use std::os::unix::fs::MetadataExt;
            metadata.dev()
        },
        #[cfg(unix)]
        inode: {
            use std::os::unix::fs::MetadataExt;
            metadata.ino()
        },
    })
}
fn path_stamp(path: &Path) -> Result<Option<Stamp>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(StorageError::PathEscape);
            }
            Ok(Some(stamp(&metadata)?))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(StorageError::io("inspect database snapshot source", error)),
    }
}
fn paths(path: &Path) -> [PathBuf; 3] {
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    let mut shm = path.as_os_str().to_os_string();
    shm.push("-shm");
    [path.to_owned(), wal.into(), shm.into()]
}

/// Rechecked byte-copy probe, not a claim that timestamp checks alone prove
/// lifetime exclusion or file ownership on any target. The retained authority
/// must already prohibit all competing mutations;4.7/4.8 supply that boundary.
pub(crate) fn probe_owned(path: &Path, staging: &Path) -> Result<SchemaPrefix> {
    let sources = paths(path);
    let before = sources
        .iter()
        .map(|path| path_stamp(path))
        .collect::<Result<Vec<_>>>()?;
    if before[0].is_none() {
        return Err(StorageError::ForeignSchema);
    }
    let scratch = tempfile::Builder::new()
        .prefix("sqlite-probe-")
        .tempdir_in(staging)
        .map_err(|e| StorageError::io("create private SQLite probe", e))?;
    for (index, source) in sources.iter().enumerate() {
        let Some(expected) = &before[index] else {
            continue;
        };
        let mut source_file =
            File::open(source).map_err(|e| StorageError::io("open SQLite probe source", e))?;
        if stamp(
            &source_file
                .metadata()
                .map_err(|e| StorageError::io("inspect open SQLite probe source", e))?,
        )? != *expected
        {
            return Err(StorageError::SnapshotChanged);
        }
        let filename = source.file_name().ok_or(StorageError::InvalidManagedPath)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut destination = options
            .open(scratch.path().join(filename))
            .map_err(|e| StorageError::io("create independent SQLite probe file", e))?;
        std::io::copy(&mut source_file, &mut destination)
            .map_err(|e| StorageError::io("copy independent SQLite probe", e))?;
        if stamp(
            &source_file
                .metadata()
                .map_err(|e| StorageError::io("recheck SQLite probe handle", e))?,
        )? != *expected
        {
            return Err(StorageError::SnapshotChanged);
        }
    }
    for (source, expected) in sources.iter().zip(&before) {
        if path_stamp(source)? != *expected {
            return Err(StorageError::SnapshotChanged);
        }
    }
    let copy = scratch
        .path()
        .join(path.file_name().ok_or(StorageError::InvalidManagedPath)?);
    let mut header = [0; 16];
    let mut header_file =
        File::open(&copy).map_err(|e| StorageError::io("open copied SQLite header", e))?;
    match header_file.read_exact(&mut header) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(StorageError::ForeignSchema);
        }
        Err(error) => return Err(StorageError::io("read copied SQLite header", error)),
    }
    drop(header_file);
    if &header != b"SQLite format 3\0" {
        return Err(StorageError::ForeignSchema);
    }
    // Read-only SQLite can update SHM/WAL. Only the independent copy is opened.
    let mut connection = Connection::open_with_flags(
        copy,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let prefix = SchemaPrefix::inspect(&mut connection)?;
    verify_integrity(&connection)?;
    connection
        .close()
        .map_err(|(_, error)| StorageError::Sqlite(error))?;
    Ok(prefix)
}

pub(crate) fn verify_integrity(connection: &Connection) -> Result<()> {
    let integrity = connection
        .prepare("PRAGMA integrity_check")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if integrity != ["ok"] {
        return Err(StorageError::IntegrityFailed);
    }
    if connection
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some()
    {
        return Err(StorageError::IntegrityFailed);
    }
    Ok(())
}
fn counts(connection: &Connection) -> Result<BTreeMap<String, i64>> {
    let tables=connection.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND substr(name,1,7)<>'sqlite_' ORDER BY name")?
        .query_map([],|row| row.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
    tables
        .into_iter()
        .map(|table| {
            let sql = format!("SELECT count(*) FROM \"{}\"", table.replace('"', "\"\""));
            let count = connection.query_row(&sql, [], |row| row.get(0))?;
            Ok((table, count))
        })
        .collect()
}

pub(crate) fn online(
    source: &Connection,
    directory: &Path,
    label: &str,
    cancellation: &WorkCancellation,
) -> Result<BackupReceipt> {
    online_with_checkpoint(
        source,
        directory,
        label,
        cancellation,
        || {},
        sync_directory,
    )
}

// The private synchronous checkpoint is a deterministic test seam, not an app
// I/O callback or an extension to any domain transaction/hook protocol.
fn online_with_checkpoint(
    source: &Connection,
    directory: &Path,
    label: &str,
    cancellation: &WorkCancellation,
    mut checkpoint: impl FnMut(),
    flush_directory: impl FnOnce(&Path) -> Result<()>,
) -> Result<BackupReceipt> {
    if cancellation.is_cancelled() {
        return Err(StorageError::Cancelled);
    }
    // Directory durability has a separate Windows capability gate. A host-only
    // no-op would falsely authorize migration;4.9 must provide real evidence.
    require_directory_durability()?;
    if !source.is_autocommit() {
        return Err(StorageError::RuntimeFenced);
    }
    let snapshot = source.unchecked_transaction()?;
    // Establish the read snapshot before backup so counts use the SAME view.
    let source_counts = counts(&snapshot)?;
    let mut reservation = tempfile::Builder::new()
        .prefix(label)
        .suffix(".sqlite3")
        .tempfile_in(directory)
        .map_err(|e| StorageError::io("reserve private online backup", e))?;
    let mut destination = Connection::open_with_flags(
        reservation.path(),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    destination.busy_timeout(BUSY_TIMEOUT)?;
    {
        let backup = Backup::new(&snapshot, &mut destination)?;
        let mut previous = None;
        loop {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let step = backup.step(64)?;
            checkpoint();
            match step {
                StepResult::Done => break,
                StepResult::More => {
                    let progress = backup.progress();
                    if previous.is_some_and(|remaining| progress.remaining >= remaining) {
                        return Err(StorageError::SnapshotChanged);
                    }
                    previous = Some(progress.remaining);
                }
                StepResult::Busy | StepResult::Locked => return Err(StorageError::BackupBusy),
                _ => return Err(StorageError::BackupBusy),
            }
        }
    }
    destination.pragma_update(None, "journal_mode", "DELETE")?;
    verify_integrity(&destination)?;
    let prefix = SchemaPrefix::inspect(&mut destination)?;
    if counts(&destination)? != source_counts {
        return Err(StorageError::IntegrityFailed);
    }
    snapshot.commit()?;
    destination
        .close()
        .map_err(|(_, error)| StorageError::Sqlite(error))?;
    // From verified complete bytes onward no failure branch may delete the
    // snapshot. Retain its actual witness even when flush/keep/reporting fails.
    reservation.disable_cleanup(true);
    let receipt = BackupReceipt {
        path: reservation.path().to_owned(),
        prefix,
        counts: source_counts,
    };
    if let Err(error) = reservation.as_file().sync_all() {
        return Err(StorageError::BackupUncertain {
            backup: Box::new(receipt),
            cause: Box::new(StorageError::io("flush standalone backup", error)),
        });
    }
    let (file, _path) = match reservation.keep() {
        Ok(kept) => kept,
        Err(error) => {
            return Err(StorageError::BackupUncertain {
                backup: Box::new(receipt),
                cause: Box::new(StorageError::io(
                    "retain verified online backup",
                    error.error,
                )),
            });
        }
    };
    drop(file);
    if let Err(cause) = flush_directory(directory) {
        return Err(StorageError::BackupUncertain {
            backup: Box::new(receipt),
            cause: Box::new(cause),
        });
    }
    Ok(receipt)
}

pub(crate) fn require_directory_durability() -> Result<()> {
    #[cfg(unix)]
    {
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(StorageError::DirectoryDurabilityUnavailable)
    }
}
pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    require_directory_durability()?;
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|e| StorageError::io("flush directory", e))
}

/// Move only into a fresh private recovery directory. Hard-link creation is an
/// atomic no-clobber namespace operation on a regular file; the original name
/// is unlinked only after its independent recovery name exists. Any partial
/// failure retains all remaining names, never overwrites or fabricates success.
pub(crate) fn preserve_candidate(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().ok_or(StorageError::InvalidManagedPath)?;
    let directory = tempfile::Builder::new()
        .prefix("migration-failed-")
        .tempdir_in(parent)
        .map_err(|e| StorageError::io("reserve failed migration recovery", e))?
        .keep();
    let preserved = (|| {
        preserve_files(path, &directory)?;
        sync_directory(&directory)?;
        sync_directory(parent)
    })();
    if let Err(cause) = preserved {
        return Err(StorageError::PreservationFailed {
            directory,
            cause: Box::new(cause),
        });
    }
    Ok(directory)
}
fn preserve_files(path: &Path, directory: &Path) -> Result<()> {
    for source in paths(path) {
        if path_stamp(&source)?.is_none() {
            continue;
        }
        let destination =
            directory.join(source.file_name().ok_or(StorageError::InvalidManagedPath)?);
        fs::hard_link(&source, &destination)
            .map_err(|e| StorageError::io("preserve candidate without clobber", e))?;
        File::open(&source)
            .and_then(|file| file.sync_all())
            .map_err(|e| StorageError::io("flush preserved candidate bytes", e))?;
        fs::remove_file(&source)
            .map_err(|e| StorageError::io("unlink preserved candidate name", e))?;
    }
    Ok(())
}

fn restore(backup: &BackupReceipt, path: &Path) -> Result<()> {
    let parent = path.parent().ok_or(StorageError::InvalidManagedPath)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".migration-rollback-")
        .tempfile_in(parent)
        .map_err(|e| StorageError::io("reserve migration rollback", e))?;
    let mut source = File::open(backup.path())
        .map_err(|e| StorageError::io("open verified rollback backup", e))?;
    std::io::copy(&mut source, &mut temporary)
        .map_err(|e| StorageError::io("copy rollback backup", e))?;
    temporary
        .flush()
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|e| StorageError::io("flush rollback backup", e))?;
    fs::hard_link(temporary.path(), path)
        .map_err(|e| StorageError::io("publish rollback without clobber", e))?;
    temporary
        .close()
        .map_err(|e| StorageError::io("remove owned rollback temporary name", e))?;
    sync_directory(parent)?;
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    verify_integrity(&connection)?;
    if SchemaPrefix::inspect(&mut connection)? != backup.prefix
        || counts(&connection)? != backup.counts
    {
        return Err(StorageError::IntegrityFailed);
    }
    connection
        .close()
        .map_err(|(_, error)| StorageError::Sqlite(error))?;
    Ok(())
}

pub(crate) fn upgrade_owned(path: &Path, staging: &Path, plan: &[Migration]) -> Result<Connection> {
    let prefix = probe_owned(path, staging)?;
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    if SchemaPrefix::inspect(&mut connection)? != prefix {
        return Err(StorageError::SnapshotChanged);
    }
    crate::database::configure_writer(&connection)?;
    let pending = plan
        .iter()
        .filter(|migration| migration.version > prefix.version())
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(connection);
    }
    let backup = online(
        &connection,
        path.parent().ok_or(StorageError::InvalidManagedPath)?,
        &format!("librairii.pre-migration-v{:03}-", pending[0].version),
        &WorkCancellation::default(),
    )?;
    let mut failure = None;
    let mut last_version = prefix.version();
    for migration in pending {
        let applied = (|| -> Result<()> {
            let transaction = connection.transaction()?;
            transaction.execute_batch(migration.sql)?;
            transaction.execute(
                "INSERT INTO schema_migrations(version) VALUES(?)",
                [migration.version],
            )?;
            transaction.commit()?;
            Ok(())
        })();
        if let Err(cause) = applied {
            failure = Some((migration.version, cause));
            break;
        }
        last_version = migration.version;
    }
    if failure.is_none() {
        let verified = (|| {
            verify_integrity(&connection)?;
            SchemaPrefix::inspect(&mut connection)?;
            Ok(())
        })();
        if let Err(error) = verified {
            failure = Some((last_version, error));
        }
    }
    if let Some((version, cause)) = failure {
        if let Err((_connection, close_error)) = connection.close() {
            return Err(StorageError::MigrationFailed {
                version,
                backup: backup.path,
                candidate: path.to_owned(),
                restored: false,
                cause: Box::new(StorageError::RollbackUncertain {
                    work: Box::new(cause),
                    rollback: close_error,
                }),
            });
        }
        let candidate = match preserve_candidate(path) {
            Ok(candidate) => candidate,
            Err(error) => {
                return Err(StorageError::MigrationFailed {
                    version,
                    backup: backup.path,
                    candidate: path.to_owned(),
                    restored: false,
                    cause: Box::new(error),
                });
            }
        };
        let restored = restore(&backup, path);
        return Err(StorageError::MigrationFailed {
            version,
            backup: backup.path,
            candidate,
            restored: restored.is_ok(),
            cause: Box::new(restored.err().unwrap_or(cause)),
        });
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use librairii_testkit::TestRoots;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[cfg(unix)]
    #[test]
    fn migration_failure_restores_supported_prefix_and_preserves_newer_candidate() -> TestResult {
        let roots = TestRoots::new()?;
        let library = crate::IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "A0000000-0000-4000-8000-000000000001".parse()?,
        )?;
        let database = library.layout().database();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../preservation/corpus/databases/prefix-001.sqlite3"),
            &database,
        )?;
        let archive = library.layout().archives().join("retained.zip");
        fs::write(&archive, b"exact immutable archive\0\xff")?;
        let plan = [
            crate::migrations::MIGRATIONS[0],
            Migration {
                version: 2,
                filename: "002_injected_committed.sql",
                sql: "CREATE TABLE newer_candidate(value BLOB); INSERT INTO newer_candidate VALUES(x'0001FF')",
            },
            Migration {
                version: 3,
                filename: "003_injected_failure.sql",
                sql: "CREATE TABLE rolled_back(value INTEGER); SELECT * FROM missing_table",
            },
        ];
        let error = upgrade_owned(&database, &library.layout().staging(), &plan)
            .err()
            .ok_or("missing injected migration failure")?;
        let StorageError::MigrationFailed {
            version,
            backup,
            candidate,
            restored,
            cause: _,
        } = error
        else {
            return Err("wrong failure disposition".into());
        };
        assert_eq!(version, 3);
        assert!(restored);
        assert!(backup.is_file());
        assert!(candidate.is_dir());
        let mut restored =
            Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        assert_eq!(SchemaPrefix::inspect(&mut restored)?.version(), 1);
        verify_integrity(&restored)?;
        drop(restored);
        let preserved = Connection::open_with_flags(
            candidate.join("librairii.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        assert_eq!(
            preserved.query_row("SELECT max(version) FROM schema_migrations", [], |row| row
                .get::<_, i64>(
                0
            ))?,
            2
        );
        assert_eq!(
            preserved.query_row("SELECT value FROM newer_candidate", [], |row| row
                .get::<_, Vec<u8>>(0))?,
            vec![0, 1, 255]
        );
        assert_eq!(
            preserved.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='rolled_back'",
                [],
                |row| row.get::<_, i64>(0)
            )?,
            0
        );
        assert_eq!(fs::read(archive)?, b"exact immutable archive\0\xff");
        Ok(())
    }

    #[test]
    fn backup_reservation_failure_does_not_modify_source_state() -> TestResult {
        let roots = TestRoots::new()?;
        let source = roots.library().join("source.sqlite3");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../preservation/corpus/databases/prefix-018.sqlite3"),
            &source,
        )?;
        let connection = Connection::open(&source)?;
        let before = counts(&connection)?;
        let result = online(
            &connection,
            &roots.library().join("missing-directory"),
            "uncreated-",
            &WorkCancellation::default(),
        );
        #[cfg(unix)]
        assert!(matches!(result, Err(StorageError::Io { .. })));
        #[cfg(not(unix))]
        assert!(matches!(
            result,
            Err(StorageError::DirectoryDurabilityUnavailable)
        ));
        assert_eq!(counts(&connection)?, before);
        verify_integrity(&connection)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_after_a_real_backup_chunk_keeps_the_source_unchanged() -> TestResult {
        let roots = TestRoots::new()?;
        let source = roots.library().join("source.sqlite3");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../preservation/corpus/databases/prefix-018.sqlite3"),
            &source,
        )?;
        let connection = Connection::open(&source)?;
        connection.execute(
            "UPDATE stories SET embedded_description=?",
            ["x".repeat(2 * 1024 * 1024)],
        )?;
        let before = fs::read(&source)?;
        let cancellation = WorkCancellation::default();
        let signal = cancellation.clone();
        let mut checkpoints = 0;
        let result = online_with_checkpoint(
            &connection,
            roots.library(),
            "cancelled-",
            &cancellation,
            || {
                checkpoints += 1;
                signal.cancel();
            },
            sync_directory,
        );
        assert!(matches!(result, Err(StorageError::Cancelled)));
        assert_eq!(checkpoints, 1);
        assert_eq!(fs::read(&source)?, before);
        assert!(
            !fs::read_dir(roots.library())?
                .collect::<std::io::Result<Vec<_>>>()?
                .iter()
                .any(|entry| entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("cancelled-")))
        );
        verify_integrity(&connection)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn post_backup_flush_failure_retains_verified_receipt_and_bytes_on_error_drop() -> TestResult {
        let roots = TestRoots::new()?;
        let source = roots.library().join("source.sqlite3");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../preservation/corpus/databases/prefix-018.sqlite3"),
            &source,
        )?;
        let before = fs::read(&source)?;
        let connection = Connection::open(&source)?;
        let error = online_with_checkpoint(
            &connection,
            roots.library(),
            "uncertain-",
            &WorkCancellation::default(),
            || {},
            |_| Err(StorageError::DirectoryDurabilityUnavailable),
        )
        .err()
        .ok_or("missing flush failure")?;
        let StorageError::BackupUncertain { backup, cause: _ } = error else {
            return Err("missing uncertainty witness".into());
        };
        let retained = backup.path().to_owned();
        assert_eq!(backup.prefix().version(), 18);
        let mut read = Connection::open_with_flags(&retained, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        verify_integrity(&read)?;
        assert_eq!(SchemaPrefix::inspect(&mut read)?.version(), 18);
        drop(read);
        drop(backup);
        assert!(retained.is_file());
        assert_eq!(fs::read(source)?, before);
        Ok(())
    }
}
