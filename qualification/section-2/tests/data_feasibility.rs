//! Candidate capability probes, not a replacement for section-1 oracle outcomes.
use rusqlite::{Connection, backup::Backup};
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

fn source(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn migrations() -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(source("internal/database/migrations"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 18);
    paths
}

fn apply(conn: &mut Connection, files: &[PathBuf], from: usize, to: usize) {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)").unwrap();
    for (index, path) in files.iter().enumerate().take(to).skip(from) {
        let transaction = conn.transaction().unwrap();
        transaction
            .execute_batch(&std::fs::read_to_string(path).unwrap())
            .unwrap();
        transaction
            .execute(
                "INSERT INTO schema_migrations(version) VALUES(?1)",
                [i64::try_from(index + 1).unwrap()],
            )
            .unwrap();
        transaction.commit().unwrap();
    }
}

#[test]
fn all_empty_sql_prefixes_reopen_and_migrate_with_bundled_sqlite() {
    let files = migrations();
    for prefix in 1..=18 {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("synthetic.sqlite3");
        let mut conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
        )
        .unwrap();
        apply(&mut conn, &files, 0, prefix);
        drop(conn);
        let mut conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")
            .unwrap();
        apply(&mut conn, &files, prefix, 18);
        let identity: (String, String) = conn
            .query_row(
                "SELECT product,schema_family FROM librairii_schema_identity WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(identity, ("librairii".into(), "wails-go-sqlite".into()));
        let ledger: i64 = conn
            .query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ledger, 18);
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        assert!(
            !conn
                .prepare("PRAGMA foreign_key_check")
                .unwrap()
                .exists([])
                .unwrap()
        );
        let json: String = conn
            .query_row("SELECT json_type('[1,2]')", [], |r| r.get(0))
            .unwrap();
        assert_eq!(json, "array");
        println!(
            "empty_prefix={prefix} reopen_integrity=ok SQLite={}",
            rusqlite::version()
        );
    }
}

#[test]
fn online_backup_includes_committed_wal_and_preserves_independent_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wal.sqlite3");
    let mut source_db = Connection::open(&path).unwrap();
    source_db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE probe(value TEXT); INSERT INTO probe VALUES('before');").unwrap();
    assert!(
        directory
            .path()
            .join("wal.sqlite3-wal")
            .metadata()
            .unwrap()
            .len()
            > 0
    );
    let mut destination = Connection::open(directory.path().join("backup.sqlite3")).unwrap();
    Backup::new(&source_db, &mut destination)
        .unwrap()
        .run_to_completion(5, Duration::from_millis(10), None)
        .unwrap();
    source_db
        .execute("UPDATE probe SET value='after'", [])
        .unwrap();
    let value: String = destination
        .query_row("SELECT value FROM probe", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "before");
    let disabled = source_db
        .query_row(
            "SELECT load_extension('qualification-nonexistent')",
            [],
            |_| Ok(()),
        )
        .unwrap_err();
    assert!(disabled.to_string().contains("not authorized"));
    {
        let transaction = source_db.transaction().unwrap();
        transaction
            .execute_batch("CREATE TABLE rolled_back(value TEXT);")
            .unwrap();
        // Dropping rolls back DDL as well as data.
    }
    let count: i64 = source_db
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name='rolled_back'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    let mut statement = source_db.prepare("PRAGMA compile_options").unwrap();
    let options: Vec<String> = statement
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    println!("SQLite={} compile_options={options:?}", rusqlite::version());
}

fn zip_bytes(method: zip::CompressionMethod, name: &str) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(method);
    archive.start_file(name, options).unwrap();
    archive.write_all(b"synthetic-payload-for-crc").unwrap();
    archive.finish().unwrap().into_inner()
}

#[test]
fn zip_store_deflate_crc_and_unsafe_name_are_observable_without_extraction() {
    for method in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let bytes = zip_bytes(method, "Été 日本語/ni");
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut entry = archive.by_index(0).unwrap();
        assert!(!entry.encrypted());
        let mut result = Vec::new();
        entry.read_to_end(&mut result).unwrap();
        assert_eq!(result, b"synthetic-payload-for-crc");
    }
    let mut corrupt = zip_bytes(zip::CompressionMethod::Stored, "ni");
    let start = corrupt
        .windows(25)
        .position(|w| w == b"synthetic-payload-for-crc")
        .unwrap();
    corrupt[start] ^= 1;
    let mut archive = zip::ZipArchive::new(Cursor::new(corrupt)).unwrap();
    assert!(
        archive
            .by_index(0)
            .unwrap()
            .read_to_end(&mut Vec::new())
            .is_err()
    );
    let bytes = zip_bytes(zip::CompressionMethod::Stored, "../outside");
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert!(archive.by_index(0).unwrap().enclosed_name().is_none());
    assert!(zip::ZipArchive::new(Cursor::new(b"not zip")).is_err());
}

#[test]
fn sevenzip_checked_in_synthetic_containers_decode_bounded_without_extraction() {
    for name in [
        "generic",
        "studio",
        "bomb",
        "deep-path",
        "symlink",
        "nested",
        "missing",
    ] {
        let path = source(&format!(
            "internal/inspection/testfixture/testdata/{name}.7z"
        ));
        let before = std::fs::read(&path).unwrap();
        let mut reader =
            sevenz_rust2::ArchiveReader::new(Cursor::new(&before), sevenz_rust2::Password::empty())
                .unwrap();
        reader.set_thread_count(1);
        let entries = reader.archive().files.len();
        assert!(entries > 0 && entries < 100);
        let mut total = 0_u64;
        reader
            .for_each_entries(|entry, input| {
                let count = std::io::copy(&mut input.take((1 << 20) + 1), &mut std::io::sink())?;
                total += count;
                if total > 1 << 20 || count != entry.size {
                    return Err(std::io::Error::other(
                        "qualification byte ceiling/length mismatch",
                    )
                    .into());
                }
                Ok(true)
            })
            .unwrap();
        assert_eq!(before, std::fs::read(&path).unwrap());
        println!(
            "7z={name} container_entries={entries} decoded_bytes={total}; product acceptance NOT evaluated"
        );
    }
}

#[test]
fn sevenzip_truncation_and_header_crc_are_rejected() {
    let bytes = std::fs::read(source(
        "internal/inspection/testfixture/testdata/generic.7z",
    ))
    .unwrap();
    assert!(
        sevenz_rust2::ArchiveReader::new(
            Cursor::new(&bytes[..12]),
            sevenz_rust2::Password::empty()
        )
        .is_err()
    );
    let mut corrupt = bytes;
    corrupt[12] ^= 1;
    assert!(
        sevenz_rust2::ArchiveReader::new(Cursor::new(corrupt), sevenz_rust2::Password::empty())
            .is_err()
    );
}
