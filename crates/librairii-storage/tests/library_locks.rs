use librairii_core::LibraryId;
use librairii_storage::{
    IsolatedLibrary, StorageError, database::Database, platform::lock::LIBRARY_LOCK_FILE,
};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{BufRead, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
};

type TestResult = Result<(), Box<dyn Error>>;

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn isolated(root: &Path) -> Result<IsolatedLibrary, Box<dyn Error>> {
    let id: LibraryId = "a0000000-0000-4000-8000-000000000001".parse()?;
    Ok(IsolatedLibrary::initialize_from_override(
        Some(root.as_os_str()),
        id,
    )?)
}

fn child_command(root: &Path, mode: &str) -> Result<Command, Box<dyn Error>> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--exact", "library_lock_child_entry", "--nocapture"])
        .env("LIBRAIRII_TEST_LIBRARY_LOCK_ROOT", root)
        .env("LIBRAIRII_TEST_LIBRARY_LOCK_MODE", mode);
    Ok(command)
}

#[test]
fn rust_lock_is_not_legacy_lifetime_exclusion_or_existing_adoption_authority() -> TestResult {
    let roots = TestRoots::new()?;
    let root = roots.library().join("existing-source");
    let library = isolated(&root)?;
    let marker = fs::read(root.join(".librairii-library"))?;
    fs::write(
        roots.profile().join("settings.json"),
        b"unchanged synthetic legacy authority",
    )?;
    assert!(matches!(
        librairii_storage::adoption::open_legacy_in_place(&root),
        Err(StorageError::LegacyLifetimeExclusionUnproven)
    ));
    assert!(matches!(
        librairii_storage::adoption::open_legacy_in_place(&roots.library().join("absent-source")),
        Err(StorageError::LegacyLifetimeExclusionUnproven)
    ));
    assert!(!roots.library().join("absent-source").exists());
    assert!(!library.layout().database().exists());
    assert_eq!(fs::read(root.join(".librairii-library"))?, marker);
    assert_eq!(
        fs::read(roots.profile().join("settings.json"))?,
        b"unchanged synthetic legacy authority"
    );
    Ok(())
}

#[test]
fn library_lease_remains_owned_through_durable_repair_and_resource_close() -> TestResult {
    let roots = TestRoots::new()?;
    let root = roots.library().join("isolated");
    let library = isolated(&root)?;
    assert!(child_command(&root, "expect_locked")?.status()?.success());
    let mut database = Database::open_isolated(&library)?;
    database.stop_ordinary()?;
    assert!(child_command(&root, "expect_locked")?.status()?.success());
    assert_eq!(
        database
            .barrier_read(|tx| Ok(tx.query_row(
                "SELECT count(*) FROM schema_migrations",
                [],
                |row| row.get::<_, i64>(0)
            )?))?
            .wait()?,
        18
    );
    database.close()?;
    assert!(child_command(&root, "expect_locked")?.status()?.success());
    drop(database);
    drop(library);
    assert!(root.join(LIBRARY_LOCK_FILE).exists());
    assert!(child_command(&root, "expect_free")?.status()?.success());
    Ok(())
}

#[test]
fn process_kill_releases_kernel_lock_without_removing_stale_lock_file() -> TestResult {
    let roots = TestRoots::new()?;
    let root = roots.library().join("crashed-owner");
    fs::create_dir(&root)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(LIBRARY_LOCK_FILE))?;
    let mut child = OwnedChild(
        child_command(&root, "hold")?
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let stdout = child.0.stdout.take().ok_or("missing child stdout")?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut lines = std::io::BufReader::new(stdout);
        let result = (|| -> std::io::Result<()> {
            loop {
                let mut line = String::new();
                if lines.read_line(&mut line)? == 0 {
                    return Err(std::io::Error::other("lock child ended before readiness"));
                }
                if line.contains("LIBRARY_LOCK_READY") {
                    return Ok(());
                }
            }
        })();
        let _ = sender.send(result);
    });
    let ready = receiver.recv_timeout(std::time::Duration::from_secs(10));
    if !matches!(ready, Ok(Ok(()))) {
        let _ = child.0.kill();
        let _ = child.0.wait();
        let _ = reader.join();
        return Err(format!("lock child readiness failed: {ready:?}").into());
    }
    reader.join().map_err(|_| "lock child reader panicked")?;
    assert!(child_command(&root, "expect_locked")?.status()?.success());
    child.0.kill()?;
    child.0.wait()?;
    assert!(root.join(LIBRARY_LOCK_FILE).exists());
    assert!(child_command(&root, "expect_free")?.status()?.success());
    Ok(())
}

#[test]
fn changed_identity_refuses_runtime_before_database_creation() -> TestResult {
    let roots = TestRoots::new()?;
    let root = roots.library().join("isolated");
    let library = isolated(&root)?;
    let changed = b"A0000000-0000-4000-8000-000000000001\n";
    fs::write(root.join(".librairii-library"), changed)?;
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::IdentityMismatch)
    ));
    assert!(!library.layout().database().exists());
    assert_eq!(fs::read(root.join(".librairii-library"))?, changed);
    Ok(())
}

#[test]
#[cfg(unix)]
fn substituted_lock_fences_new_admissions_without_claiming_pending_effects() -> TestResult {
    let roots = TestRoots::new()?;
    let root = roots.library().join("isolated");
    let library = isolated(&root)?;
    let mut database = Database::open_isolated(&library)?;
    let original = root.join("retained-old-lock");
    fs::rename(root.join(LIBRARY_LOCK_FILE), &original)?;
    fs::write(root.join(LIBRARY_LOCK_FILE), b"")?;
    assert!(matches!(
        database.read(|_| Ok(())),
        Err(StorageError::LibraryLeaseChanged)
    ));
    assert!(matches!(
        library.revalidate_ownership(),
        Err(StorageError::LibraryLeaseChanged)
    ));
    // Restore the actual retained name under test ownership, then required
    // authoritative readback is usable; ordinary intake stays fenced/stopped.
    fs::remove_file(root.join(LIBRARY_LOCK_FILE))?;
    fs::rename(original, root.join(LIBRARY_LOCK_FILE))?;
    library.revalidate_ownership()?;
    assert!(database.barrier_read(|_| Ok(true))?.wait()?);
    assert!(matches!(
        database.read(|_| Ok(())),
        Err(StorageError::IntakeClosed)
    ));
    database.close()?;
    Ok(())
}

#[test]
fn library_lock_child_entry() -> TestResult {
    let Some(root) = std::env::var_os("LIBRAIRII_TEST_LIBRARY_LOCK_ROOT") else {
        return Ok(());
    };
    let mode = std::env::var("LIBRAIRII_TEST_LIBRARY_LOCK_MODE")?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(Path::new(&root).join(LIBRARY_LOCK_FILE))?;
    let result = file.try_lock();
    match mode.as_str() {
        "expect_locked" => assert!(matches!(result, Err(fs::TryLockError::WouldBlock))),
        "expect_free" => result?,
        "hold" => {
            result?;
            println!("LIBRARY_LOCK_READY");
            std::io::stdout().flush()?;
            let mut byte = [0u8; 1];
            let _ = std::io::stdin().read(&mut byte)?;
        }
        _ => return Err("unknown lock child mode".into()),
    }
    Ok(())
}
