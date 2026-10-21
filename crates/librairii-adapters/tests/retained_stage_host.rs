#![cfg(target_os = "macos")]

// One isolated qualification attempt, not a production resource limit.
// Only the dedicated child changes its soft descriptor limit. All byte and
// ownership observations happen after its filler handles have been released.
use librairii_adapters::archive::custody::{
    CreationOutcome, QualificationEnvelope, copy_created_stage,
};
use librairii_storage::{
    StorageError,
    adoption::{ClosedLibraryInput, ClosedManifest, IndependentCopy},
    database::{Database, WorkCancellation},
    native::NativeLibrary,
    platform::{
        hash_stream,
        unix::{EntryName, FileArea, LibraryFiles, StageCreateDisposition},
    },
    profile::{NativeProfileStore, ProfileDocument, ProfileLocations},
};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    ffi::OsStr,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const ENTRY: &str = "retained_stage_emfile_routes_one_real_native_failure";
const CHILD_ROLE: &str = "LIBRAIRII_G2_EMFILE_CHILD";
const ROLE_VALUE: &str = "one-fixed-512-attempt";
const FD_CAP: usize = 512;
// Selected macOS ABI: the root-inspected MacOSX26.2 SDK sys/errno.h has
// ENFILE=23 (line113) and EMFILE=24 (line114), file SHA256
// 109ace10e79b9467dee3b8c7890f1d5bea1102d982c521922e9d372ab9863c97.
// This is not a deployed-libc FD-peak assertion.
const EMFILE: i32 = 24;
const OUTPUT_BOUND: u64 = 64 * 1024;
const COMPLETE: &str = "G2_REAL_EMFILE_PROOF_COMPLETE";
const SOURCE_NAME: &str = "EMFILE # %2F source.BIN";
const SOURCE_BYTES: &[u8] = b"one owned source\0unchanged raw bytes\n";
const NATIVE_RECORD: &str = ".librairii-native-state.json";
const MANIFEST: &str = include_str!("../../../preservation/corpus/profiles/library-manifest.json");
const CLOSED_FILES: &[&str] = &[
    ".librairii-library",
    "archives/8d81e28a0e11786c91b6f804ee3a3b1c166507be676d629b9abb554e78afffae.zip",
    "archives/f0b0371c3b9c9773085653f76f64d210396b2a7feb857aa17b124249c1b50baa.zip",
    "archives/f410b25cce10902651591b3b981f02322b7534b4838fbc12601572213d7e29a0.zip",
    "archives/f6c1647ae18c7b94580a8805880ac3c236b27b68cf44129d41c5f4648ff08443.zip",
    "archives/ff62794cc6fc1bfb4f365d5f0f68bd93047eaac9168508503fd5d0721f04ee2d.zip",
    "db/librairii.sqlite3",
    "history.json",
    "state.json",
];
const LAUNCHER: &str = r#"builtin ulimit -S -n 512 || exit 121
[[ "$(builtin ulimit -S -n)" == 512 ]] || exit 122
builtin printf 'verified_child_soft_fd_cap=512\n'
builtin exec "$@"
"#;

// Authored synthetic-fixture encoding, not a new product JSON codec.
fn quoted(value: &str) -> String {
    let mut output = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            c if c <= '\u{1f}' => output.push_str(&format!("\\u{:04x}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn prepare(roots: &TestRoots) -> TestResult<(IndependentCopy, ProfileLocations)> {
    let source = roots.library().join("closed-synthetic-input");
    fs::create_dir(&source)?;
    fs::create_dir(source.join("archives"))?;
    fs::create_dir(source.join("db"))?;
    let corpus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/libraries/schema018");
    for relative in CLOSED_FILES {
        fs::copy(corpus.join(relative), source.join(relative))?;
    }
    fs::write(source.join("archives").join(SOURCE_NAME), SOURCE_BYTES)?;
    let mut bytes = SOURCE_BYTES;
    let digest = hash_stream(
        &mut bytes,
        SOURCE_BYTES.len() as u64,
        &WorkCancellation::default(),
    )?;
    let manifest = format!(
        "{},{{\"path\":{},\"size\":{},\"sha256\":{}}}]",
        MANIFEST
            .trim_end()
            .strip_suffix(']')
            .ok_or("manifest array")?,
        quoted(&format!("archives/{SOURCE_NAME}")),
        SOURCE_BYTES.len(),
        quoted(&digest.checksum().to_string())
    );
    let profile = format!(
        "{{\"version\":1,\"language\":\"en\",\"root\":{},\"libraryId\":\"a0000000-0000-4000-8000-000000000001\",\"complete\":true}}",
        quoted(source.to_str().ok_or("synthetic UTF8 root")?)
    );
    let cancellation = WorkCancellation::default();
    let input = ClosedLibraryInput::verify(
        &source,
        ProfileDocument::decode(profile.as_bytes())?,
        ClosedManifest::decode(manifest.as_bytes())?,
        &cancellation,
    )?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("namespace parent")?)?;
    let legacy = locations.legacy.as_ref().ok_or("legacy context")?;
    fs::create_dir_all(legacy)?;
    fs::write(legacy.join("settings.json"), profile.as_bytes())?;
    let copied = input.copy_to_new_root(&locations.data.join("one-owned-copy"), &cancellation);
    // Keep the actual typed setup owner through the failing assertion.
    assert!(
        copied.is_ok(),
        "SETUP verified independent copy: {:?}",
        copied.as_ref().err()
    );
    let copy = match copied {
        Ok(copy) => copy,
        Err(retained) => {
            std::hint::black_box(&retained);
            return Err("SETUP verified independent copy failed".into());
        }
    };
    Ok((copy, locations))
}

fn child_attempt() -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots)?;
    let source_root = copy.source_profile().profile().root.clone();
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let binding =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default());
    assert!(
        binding.is_ok(),
        "SETUP native binding: {:?}",
        binding.as_ref().err()
    );
    let native = match binding {
        Ok(native) => native,
        Err(retained) => {
            std::hint::black_box(&retained);
            return Err("SETUP native binding failed".into());
        }
    };
    // open_native synchronously acknowledges the writer and all four readers.
    // No SQL job, Pending, close/checkpoint, runtime or log consumer is started
    // while the following critical window is active.
    let database = Database::open_native(&native)?;
    assert_eq!(database.diagnostics().len(), 5);
    let files = LibraryFiles::open_native(&database)?;
    let guard = files.try_custody()?;
    assert!(matches!(
        files.try_custody(),
        Err(StorageError::CustodyBusy)
    ));
    let source = files.open_read_source(FileArea::Archives, EntryName::parse(SOURCE_NAME)?)?;
    let initial = source.initial();
    let raw_name = source.name().as_os_str().to_owned();
    assert_eq!(raw_name.as_os_str(), OsStr::new(SOURCE_NAME));
    let record_before = fs::read(root.join(NATIVE_RECORD))?;
    let original_source = fs::read(Path::new(&source_root).join("archives").join(SOURCE_NAME))?;
    assert_eq!(original_source, SOURCE_BYTES);
    let envelope = QualificationEnvelope::default();
    let cancellation = WorkCancellation::default();
    let mut fillers = Vec::with_capacity(FD_CAP);
    let mut terminal = None;
    for _ in 0..=FD_CAP {
        match File::open("/dev/null") {
            Ok(file) if fillers.len() < FD_CAP => fillers.push(file),
            Ok(file) => {
                drop(file);
                drop(fillers);
                return Err("SETUP terminal open unexpectedly succeeded after 512 fillers".into());
            }
            Err(error) => {
                terminal = Some(error);
                break;
            }
        }
    }
    let terminal = match terminal {
        Some(error) if error.raw_os_error() == Some(EMFILE) && fillers.len() >= 2 => error,
        other => {
            drop(fillers);
            return Err(format!("SETUP exhaustion was not usable actual EMFILE: {other:?}").into());
        }
    };
    let filler_count = fillers.len();

    // CRITICAL WINDOW: exactly two slots, one unchanged public factory call.
    // No logging, assertion, getter, FD census or other I/O until restoration.
    drop(fillers.pop());
    drop(fillers.pop());
    let creation = files.create_stage(&guard);
    drop(fillers);

    // Pure address capture only, after restoration and immediately before the
    // unchanged G2 route. Nothing dereferences a raw pointer or mints authority.
    let original_box = creation
        .as_ref()
        .err()
        .map(|failure| std::ptr::from_ref(failure.as_ref()));
    let mut outcome = copy_created_stage(source, creation, guard, envelope, &cancellation);

    // All assertions now borrow the physically retained whole outcome.
    let owned = match &mut outcome {
        CreationOutcome::CreationFailed(owned) => owned,
        CreationOutcome::CopyAttempt(_) => {
            return Err("factory succeeded; this fixed two-slot fixture is unqualified".into());
        }
    };
    assert_eq!(
        original_box,
        Some(std::ptr::from_ref(owned.failure.as_ref()))
    );
    assert_eq!(
        owned.failure.disposition(),
        StageCreateDisposition::RetainedUnresolved
    );
    assert!(
        matches!(owned.failure.cause(), StorageError::Io { .. }),
        "actual final witness cause: {:?}",
        owned.failure.cause()
    );
    let (operation, raw_errno) = match owned.failure.cause() {
        StorageError::Io { operation, source } => (*operation, source.raw_os_error()),
        _ => return Err("actual final witness failure was not structured I/O".into()),
    };
    assert_eq!(raw_errno, Some(EMFILE));
    let allocation = owned.failure.retained_allocation();
    assert!(allocation.is_some(), "actual retained stage allocation");
    let Some(allocation) = allocation else {
        return Err("retained allocation absent".into());
    };
    assert!(allocation.mkdir_attempted());
    assert!(allocation.directory_created());
    assert!(allocation.payload_open_attempted());
    assert!(allocation.payload_created());
    assert!(allocation.directory_acquired());
    assert!(allocation.payload_acquired());
    let directory_id = allocation.directory_identity();
    let payload_id = allocation.payload_identity();
    assert!(directory_id.is_some(), "acquired directory ID");
    assert!(payload_id.is_some(), "acquired payload ID");
    let (Some(directory_id), Some(payload_id)) = (directory_id, payload_id) else {
        return Err("acquired stage IDs absent".into());
    };
    assert_ne!(directory_id, payload_id);
    assert_ne!(allocation.parent_identity(), directory_id);
    assert_eq!(owned.source.initial(), initial);
    assert_eq!(owned.source.name().as_os_str(), raw_name.as_os_str());
    assert!(!owned.cancellation_requested);
    assert!(matches!(
        files.try_custody(),
        Err(StorageError::CustodyRecoveryRequired)
    ));
    assert!(matches!(
        NativeProfileStore::open_native(&locations.native),
        Err(StorageError::ProfileInUse)
    ));
    let payload_path = root
        .join("staging")
        .join(allocation.directory_name())
        .join(allocation.payload_name());
    // These are read-only observations in the controlled synthetic fixture,
    // not pathname-derived allocation, cleanup or replacement authority.
    assert!(fs::symlink_metadata(payload_path.parent().ok_or("stage parent absent")?)?.is_dir());
    let metadata = fs::symlink_metadata(&payload_path)?;
    assert!(metadata.is_file());
    assert_eq!(metadata.len(), 0);
    assert!(fs::read(&payload_path)?.is_empty());
    owned.source.seek(SeekFrom::Start(0))?;
    let mut source_after = Vec::with_capacity(SOURCE_BYTES.len() + 1);
    (&mut owned.source)
        .take(SOURCE_BYTES.len() as u64 + 1)
        .read_to_end(&mut source_after)?;
    assert_eq!(source_after, SOURCE_BYTES);
    assert_eq!(
        fs::read(Path::new(&source_root).join("archives").join(SOURCE_NAME))?,
        original_source
    );
    assert_eq!(fs::read(root.join(NATIVE_RECORD))?, record_before);
    println!(
        "{COMPLETE}\nsource=ba2dd0f5dc40f58fd206850d014abdfb6243f3cf\nos={}\nsoft_cap={FD_CAP}\nowned_fillers={filler_count}\nterminal_errno={:?}\noperation={operation}\nstage_errno={raw_errno:?}\nmkdir_attempted=true\ndirectory_created=true\npayload_open_attempted=true\npayload_created=true\ndirectory_acquired=true\npayload_acquired=true\nparent_id={:?}\ndirectory_id={directory_id:?}\npayload_id={payload_id:?}\nraw_source_name={raw_name:?}\nsource_initial={initial:?}\nbox_identity_preserved=true\nsource_bytes_unchanged=true\npartial_payload_bytes=0\nnative_record_unchanged=true\ncustody_owner_physically_retained=true\nnew_custody_refused_by_fence=true\nnamespace_lease_still_held=true\nsettlement=UNQUALIFIED",
        std::env::consts::OS,
        terminal.raw_os_error(),
        allocation.parent_identity(),
    );
    // There is no public custody-active getter. The original move-only guard
    // is physically held here; RecoveryRequired observes the sticky fence,
    // not an independent guard-bit predicate. No cleanup/finalizer is called.
    std::hint::black_box((&outcome, &files, &database, &native, &store));
    // Child exit and automatic negative-fixture teardown restore resources
    // only. They do not settle partial bytes or mint a clean Native restart.
    Ok(())
}

struct ProcessOwner {
    child: Child,
    reaped: bool,
}
impl ProcessOwner {
    fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.child.wait()?;
        self.reaped = true;
        Ok(status)
    }
    fn stop_and_wait(&mut self) -> io::Result<ExitStatus> {
        let _ = self.child.kill();
        self.wait()
    }
}
impl Drop for ProcessOwner {
    fn drop(&mut self) {
        if !self.reaped {
            // Fallback only, never evidence of a successful reap/settlement.
            let _ = self.child.kill();
            if self.child.wait().is_ok() {
                self.reaped = true;
            }
        }
    }
}

fn parent_attempt() -> TestResult {
    let roots = TestRoots::new()?;
    let path = roots.profile().join("one-child-output.log");
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let executable = std::env::current_exe()?;
    assert!(executable.is_absolute());
    let mut owner = ProcessOwner {
        child: Command::new("/bin/bash")
            .args([
                "--noprofile",
                "--norc",
                "-c",
                LAUNCHER,
                "g2-emfile-launcher",
            ])
            .arg(executable)
            .args(["--exact", ENTRY, "--test-threads=1", "--nocapture"])
            .env_clear()
            .env(CHILD_ROLE, ROLE_VALUE)
            .stdin(Stdio::null())
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::from(output))
            .spawn()?,
        reaped: false,
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    let (status, failure) = loop {
        match owner.child.try_wait() {
            Ok(Some(_)) => break (owner.wait()?, None),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                break (
                    owner.stop_and_wait()?,
                    Some(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "30s child deadline expired",
                    )),
                );
            }
            Err(error) => break (owner.stop_and_wait()?, Some(error)),
        }
    };
    assert!(owner.reaped);
    let mut bytes = Vec::new();
    File::open(&path)?
        .take(OUTPUT_BOUND + 1)
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() as u64 > OUTPUT_BOUND;
    bytes.truncate(OUTPUT_BOUND as usize);
    let report = String::from_utf8_lossy(&bytes);
    eprintln!(
        "one_child_status={status}; actual_reap=true; deadline_failure={failure:?}; output_truncated={truncated}\n{report}"
    );
    if failure.is_some() || !status.success() || truncated || !report.contains(COMPLETE) {
        return Err("single child fixture failed; no retry or cap/slot tuning permitted".into());
    }
    Ok(())
}

#[test]
fn retained_stage_emfile_routes_one_real_native_failure() -> TestResult {
    match std::env::var_os(CHILD_ROLE) {
        None => parent_attempt(),
        Some(role) if role.as_os_str() == OsStr::new(ROLE_VALUE) => child_attempt(),
        Some(_) => Err("invalid dedicated child role".into()),
    }
}
