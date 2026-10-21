use super::*;
use librairii_storage::{
    adoption::{ClosedLibraryInput, ClosedManifest, IndependentCopy},
    database::Database,
    native::NativeLibrary,
    platform::unix::{EntryName, FileArea, LibraryFiles, StageCreateDisposition},
    profile::{NativeProfileStore, ProfileDocument, ProfileLocations},
};
use librairii_testkit::TestRoots;
use std::{error::Error, ffi::OsString, fs, path::Path};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const MANIFEST: &str =
    include_str!("../../../../../preservation/corpus/profiles/library-manifest.json");
const SOURCE_NAME: &str = "Éte # %2F original.BIN";

// These are the exact files in the preserved manifest. Only independently
// copied test bytes are extended; no corpus/generator/profile is modified.
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

// Authored fixture encoding only, not product JSON/metadata behavior.
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

fn selected_name(root: &Path) -> TestResult<OsString> {
    for entry in fs::read_dir(root.join("archives"))? {
        let name = entry?.file_name();
        if name.as_encoded_bytes().ends_with(b".BIN") {
            return Ok(name);
        }
    }
    Err("synthetic managed source absent".into())
}

fn prepare(roots: &TestRoots, bytes: &[u8]) -> TestResult<(IndependentCopy, ProfileLocations)> {
    let source = roots.library().join("closed-synthetic-input");
    fs::create_dir(&source)?;
    fs::create_dir(source.join("archives"))?;
    fs::create_dir(source.join("db"))?;
    let corpus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/libraries/schema018");
    for relative in CLOSED_FILES {
        fs::copy(corpus.join(relative), source.join(relative))?;
    }
    // Install the explicitly synthetic managed source BEFORE genuine closed
    // verification and independent cloning. No arbitrary File becomes a source.
    fs::write(source.join("archives").join(SOURCE_NAME), bytes)?;
    // Capture the actual native directory spelling; do not assume a filesystem
    // preserves a requested Unicode normalization form or normalize it here.
    let actual_name = selected_name(&source)?;
    let mut input_bytes = bytes;
    let measurement = hash_stream(
        &mut input_bytes,
        bytes.len() as u64,
        &WorkCancellation::default(),
    )?;
    let manifest = format!(
        "{},{{\"path\":{},\"size\":{},\"sha256\":{}}}]",
        MANIFEST
            .trim_end()
            .strip_suffix(']')
            .ok_or("closed manifest array")?,
        quoted(&format!(
            "archives/{}",
            actual_name.to_str().ok_or("synthetic name UTF8")?
        )),
        bytes.len(),
        quoted(&measurement.checksum().to_string())
    );
    let profile = format!(
        "{{\"version\":1,\"language\":\"en\",\"root\":{},\"libraryId\":\"a0000000-0000-4000-8000-000000000001\",\"complete\":true}}",
        quoted(source.to_str().ok_or("test root UTF8")?)
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
    fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
    let legacy = locations.legacy.as_ref().ok_or("legacy test context")?;
    fs::create_dir_all(legacy)?;
    fs::write(legacy.join("settings.json"), profile.as_bytes())?;
    let copy = input
        .copy_to_new_root(&locations.data.join("owned-copy # %2F"), &cancellation)
        .map_err(|failure| failure.to_string())?;
    Ok((copy, locations))
}

fn with_native(
    bytes: &[u8],
    run: impl FnOnce(&LibraryFiles<'_>, &Database<'_>, &Path) -> TestResult,
) -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, bytes)?;
    let root = copy.root().to_path_buf();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut database = Database::open_native(&native)?;
    {
        let files = LibraryFiles::open_native(&database)?;
        run(&files, &database, &root)?;
    }
    database.close()?;
    drop(database);
    // Test teardown closes descriptors and preserves bytes. It is NOT a clean
    // native finalization: stage/trash artifacts remain recoverable, not purged.
    let failure = native
        .finish()
        .err()
        .ok_or("artifacts must prevent clean finalization")?;
    assert_eq!(failure.retained_library().layout().root(), root);
    Ok(())
}

fn issued<'files, 'library>(
    files: &'files LibraryFiles<'library>,
    root: &Path,
) -> TestResult<(
    OwnedReadSource<'files, 'library>,
    OwnedStage<'files, 'library>,
    Custody<'library>,
)> {
    let guard = files.try_custody()?;
    let source =
        files.open_read_source(FileArea::Archives, EntryName::parse(selected_name(root)?)?)?;
    // Only the actually successful constructor enters the mechanism proof.
    let creation = files.create_stage(&guard);
    // Positive fixture setup is not a production error-erasing adapter. An
    // unexpected failure fails this test BEFORE destructuring, never converts
    // a borrowed StageCreateFailure to a static Error/StorageError/string.
    assert!(creation.is_ok(), "positive stage fixture: {creation:?}");
    let Ok(stage) = creation else {
        return Err("positive stage fixture assertion did not hold".into());
    };
    Ok((source, stage, guard))
}

#[test]
fn prepublication_copy_retains_real_native_owners_raw_name_and_exact_eof_bytes() -> TestResult {
    let bytes: Vec<u8> = (0..(HASH_BUFFER_BYTES * 2 + 37))
        .map(|i| (i % 251) as u8)
        .collect();
    with_native(&bytes, |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let initial = source.initial();
        let mut result = copy_to_stage(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &WorkCancellation::default(),
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        let raw_name = selected_name(root)?;
        assert_eq!(result.source.name().as_os_str(), raw_name.as_os_str());
        assert_eq!(result.source.revalidate()?, initial);
        assert_eq!(result.progress.written_bytes, bytes.len() as u64);
        assert_eq!(result.progress.read_bytes, bytes.len() as u64);
        assert!(result.progress.source_eof);
        let StageOutcome::Measured { stage, measurement } = &mut result.stage else {
            return Err("not measured".into());
        };
        let mut actual = Vec::new();
        stage.inspector().rewind()?;
        stage.inspector().read_to_end(&mut actual)?;
        assert_eq!(actual, bytes);
        assert_eq!(
            *measurement,
            hash_stream(
                &mut bytes.as_slice(),
                bytes.len() as u64,
                &WorkCancellation::default()
            )?
        );
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyBusy)
        ));
        assert_eq!(fs::read(root.join("archives").join(SOURCE_NAME))?, bytes);
        Ok(())
    })
}

#[test]
fn prepublication_empty_source_requires_actual_eof_and_empty_stage_digest() -> TestResult {
    with_native(b"", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let result = copy_to_stage(
            source,
            stage,
            guard,
            QualificationEnvelope {
                source_bytes: Some(0),
            },
            &WorkCancellation::default(),
        );
        let StageOutcome::Measured { measurement, .. } = &result.stage else {
            return Err("not measured".into());
        };
        assert_eq!(measurement.bytes(), 0);
        assert_eq!(
            result.progress,
            CopyProgress {
                source_eof: true,
                ..CopyProgress::default()
            }
        );
        assert!(result.error.is_none());
        Ok(())
    })
}

#[test]
fn prepublication_growth_sentinel_is_read_but_never_written_past_bound() -> TestResult {
    with_native(b"abcdef", |files, _db, root| {
        let (mut source, mut stage, guard) = issued(files, root)?;
        let mut progress = CopyProgress::default();
        // Exercise the inner defensive bound against a real larger source,
        // not a fabricated FD or byte-returning mock. The public route always
        // derives expected length from the actual initial observation.
        let error = copy_stream(
            &mut source,
            &mut stage,
            5,
            &WorkCancellation::default(),
            &mut DirectIo,
            &mut progress,
        );
        assert!(matches!(
            error,
            Err(CopyError::LengthChanged {
                expected: 5,
                observed: 6
            })
        ));
        assert_eq!(progress.read_bytes, 6);
        assert_eq!(progress.written_bytes, 5);
        let mut actual = Vec::new();
        stage.inspector().rewind()?;
        stage.inspector().read_to_end(&mut actual)?;
        assert_eq!(actual, b"abcde");
        drop((source, stage, guard));
        Ok(())
    })
}

#[test]
fn prepublication_nonempty_stage_is_refused_without_appending_source() -> TestResult {
    with_native(b"new source", |files, _db, root| {
        let (source, mut stage, guard) = issued(files, root)?;
        stage.write_all(b"prior owned bytes")?;
        let result = copy_to_stage(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &WorkCancellation::default(),
        );
        assert!(matches!(result.error, Some(CopyError::StageNotEmpty)));
        assert_eq!(result.progress, CopyProgress::default());
        Ok(())
    })
}

// Private qualification seam: every read/write borrows the real storage owner.
// Injected EOF/errors/cancel can stop/shorten real operations, never substitute
// a fabricated source/stage, hash result or cleanup authority.
struct FaultIo<'a> {
    cancellation: &'a WorkCancellation,
    copy_reads: u64,
    hash_reads: u64,
    writes: u64,
    read_chunk: usize,
    write_chunk: usize,
    eof_after: Option<u64>,
    read_error_after: Option<u64>,
    hash_eof_after: Option<u64>,
    hash_error: bool,
    write_error_after: Option<u64>,
    unacknowledged_write: usize,
    cancel_after_read: bool,
    cancel_after_write: Option<u64>,
    cancel_phase: Option<CopyPhase>,
    interrupt_once: bool,
    checkpoint: Option<Box<dyn FnMut(CopyPhase) + 'a>>,
}
impl<'a> FaultIo<'a> {
    fn new(cancellation: &'a WorkCancellation) -> Self {
        Self {
            cancellation,
            copy_reads: 0,
            hash_reads: 0,
            writes: 0,
            read_chunk: usize::MAX,
            write_chunk: usize::MAX,
            eof_after: None,
            read_error_after: None,
            hash_eof_after: None,
            hash_error: false,
            write_error_after: None,
            unacknowledged_write: 0,
            cancel_after_read: false,
            cancel_after_write: None,
            cancel_phase: None,
            interrupt_once: false,
            checkpoint: None,
        }
    }
}
impl CopyIo for FaultIo<'_> {
    fn checkpoint(&mut self, phase: CopyPhase) {
        if let Some(action) = &mut self.checkpoint {
            action(phase);
        }
        if self.cancel_phase == Some(phase) {
            self.cancellation.cancel();
        }
    }
    fn read(
        &mut self,
        source: &mut OwnedReadSource<'_, '_>,
        phase: CopyPhase,
        bytes: &mut [u8],
    ) -> io::Result<usize> {
        if phase == CopyPhase::HashBefore {
            if self.hash_error {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected hash read",
                ));
            }
            let request = if let Some(limit) = self.hash_eof_after {
                if self.hash_reads >= limit {
                    return Ok(0);
                }
                bytes.len().min((limit - self.hash_reads) as usize)
            } else {
                bytes.len()
            };
            let count = source.read(&mut bytes[..request])?;
            self.hash_reads += count as u64;
            return Ok(count);
        }
        if phase != CopyPhase::Copy {
            return source.read(bytes);
        }
        if self.interrupt_once {
            self.interrupt_once = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self
            .read_error_after
            .is_some_and(|limit| self.copy_reads >= limit)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected copy read",
            ));
        }
        let mut request = bytes.len().min(self.read_chunk);
        if let Some(limit) = self.eof_after {
            if self.copy_reads >= limit {
                return Ok(0);
            }
            request = request.min((limit - self.copy_reads) as usize);
        }
        if let Some(limit) = self.read_error_after {
            request = request.min((limit - self.copy_reads) as usize);
        }
        let count = source.read(&mut bytes[..request])?;
        self.copy_reads += count as u64;
        if self.cancel_after_read {
            self.cancellation.cancel();
        }
        Ok(count)
    }
    fn write(&mut self, stage: &mut OwnedStage<'_, '_>, bytes: &[u8]) -> io::Result<usize> {
        if self.unacknowledged_write != 0 {
            let count = bytes.len().min(self.unacknowledged_write);
            stage.write_all(&bytes[..count])?;
            self.unacknowledged_write = 0;
            self.cancellation.cancel();
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "write failed after physical prefix",
            ));
        }
        if self
            .write_error_after
            .is_some_and(|limit| self.writes >= limit)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected stage write",
            ));
        }
        let mut request = bytes.len().min(self.write_chunk);
        if let Some(limit) = self.write_error_after {
            request = request.min((limit - self.writes) as usize);
        }
        let count = stage.write(&bytes[..request])?;
        self.writes += count as u64;
        if self
            .cancel_after_write
            .is_some_and(|limit| self.writes >= limit)
        {
            self.cancellation.cancel();
        }
        Ok(count)
    }
}

fn physical_prefix(result: &mut CopyOutcome<'_, '_>, root: &Path) -> TestResult<Vec<u8>> {
    match &mut result.stage {
        StageOutcome::Retired(retired) => {
            retired.revalidate(&result.custody)?;
            Ok(fs::read(
                root.join("trash")
                    .join(retired.trash_name().as_os_str())
                    .join("payload"),
            )?)
        }
        StageOutcome::Measured { stage, .. } => {
            let mut bytes = Vec::new();
            stage.inspector().rewind()?;
            stage.inspector().read_to_end(&mut bytes)?;
            Ok(bytes)
        }
        StageOutcome::RetirementFailed(failure) => {
            let mut bytes = Vec::new();
            failure.staged.inspector().rewind()?;
            failure.staged.inspector().read_to_end(&mut bytes)?;
            Ok(bytes)
        }
    }
}

#[test]
fn prepublication_qualification_envelope_is_not_a_product_or_expanded_limit() -> TestResult {
    with_native(b"sixsix", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let mut result = copy_to_stage(
            source,
            stage,
            guard,
            QualificationEnvelope {
                source_bytes: Some(5),
            },
            &WorkCancellation::default(),
        );
        assert!(matches!(
            result.error,
            Some(CopyError::QualificationLimit {
                observed: 6,
                limit: 5
            })
        ));
        assert_eq!(result.progress, CopyProgress::default());
        assert_eq!(physical_prefix(&mut result, root)?, b"");
        assert_eq!(
            fs::read(root.join("archives").join(SOURCE_NAME))?,
            b"sixsix"
        );
        Ok(())
    })
}

#[test]
fn prepublication_short_copy_eof_retires_only_actual_owned_prefix() -> TestResult {
    with_native(b"0123456789abcdef", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let cancel = WorkCancellation::default();
        let mut fault = FaultIo::new(&cancel);
        fault.eof_after = Some(7);
        let mut result = copy_with_io(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &cancel,
            &mut fault,
        );
        assert!(matches!(
            result.error,
            Some(CopyError::LengthChanged {
                expected: 16,
                observed: 7
            })
        ));
        assert_eq!(result.progress.written_bytes, 7);
        assert!(result.progress.source_eof);
        assert_eq!(physical_prefix(&mut result, root)?, b"0123456");
        assert_eq!(
            fs::read(root.join("archives").join(SOURCE_NAME))?,
            b"0123456789abcdef"
        );
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyBusy)
        ));
        Ok(())
    })
}

#[test]
fn prepublication_real_borrowed_read_error_preserves_prefix_and_uncancelled_retirement()
-> TestResult {
    with_native(b"0123456789abcdef", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let cancel = WorkCancellation::default();
        let mut fault = FaultIo::new(&cancel);
        fault.read_error_after = Some(7);
        fault.cancel_phase = Some(CopyPhase::RetireUnused);
        let mut result = copy_with_io(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &cancel,
            &mut fault,
        );
        assert!(
            matches!(&result.error, Some(CopyError::Io { cause, .. }) if cause.kind() == io::ErrorKind::PermissionDenied)
        );
        assert!(result.cancellation_requested);
        assert_eq!(result.progress.written_bytes, 7);
        assert_eq!(physical_prefix(&mut result, root)?, b"0123456");
        assert!(matches!(result.stage, StageOutcome::Retired(_)));
        assert_eq!(
            fs::read(root.join("archives").join(SOURCE_NAME))?,
            b"0123456789abcdef"
        );
        Ok(())
    })
}

#[test]
fn prepublication_unacknowledged_write_prefix_and_cancel_never_erase_io_cause() -> TestResult {
    with_native(b"0123456789abcdef", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let cancel = WorkCancellation::default();
        let mut fault = FaultIo::new(&cancel);
        fault.unacknowledged_write = 3;
        let mut result = copy_with_io(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &cancel,
            &mut fault,
        );
        assert!(
            matches!(&result.error, Some(CopyError::Io { cause, .. }) if cause.kind() == io::ErrorKind::PermissionDenied)
        );
        assert!(result.cancellation_requested);
        assert_eq!(result.progress.written_bytes, 0);
        assert!(result.progress.write_error_may_have_written);
        assert_eq!(physical_prefix(&mut result, root)?, b"012");
        assert_eq!(
            fs::read(root.join("archives").join(SOURCE_NAME))?,
            b"0123456789abcdef"
        );
        Ok(())
    })
}

#[test]
fn prepublication_short_writes_and_interrupted_read_still_measure_real_eof() -> TestResult {
    with_native(b"0123456789abcdef", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let cancel = WorkCancellation::default();
        let mut fault = FaultIo::new(&cancel);
        fault.read_chunk = 5;
        fault.write_chunk = 2;
        fault.interrupt_once = true;
        let mut result = copy_with_io(
            source,
            stage,
            guard,
            QualificationEnvelope {
                source_bytes: Some(16),
            },
            &cancel,
            &mut fault,
        );
        assert!(result.error.is_none());
        assert_eq!(result.progress.written_bytes, 16);
        assert_eq!(physical_prefix(&mut result, root)?, b"0123456789abcdef");
        assert!(matches!(result.stage, StageOutcome::Measured { .. }));
        Ok(())
    })
}

#[test]
fn prepublication_acknowledged_partial_write_error_keeps_exact_bytes() -> TestResult {
    with_native(b"0123456789abcdef", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let cancel = WorkCancellation::default();
        let mut fault = FaultIo::new(&cancel);
        fault.write_error_after = Some(5);
        let mut result = copy_with_io(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &cancel,
            &mut fault,
        );
        assert!(matches!(
            &result.error,
            Some(CopyError::Io {
                operation: "write stage",
                ..
            })
        ));
        assert_eq!(result.progress.written_bytes, 5);
        assert_eq!(physical_prefix(&mut result, root)?, b"01234");
        assert!(!result.cancellation_requested);
        Ok(())
    })
}

#[test]
fn prepublication_cancellation_phase_matrix_retains_owners_without_partial_digest() -> TestResult {
    for (phase, prefix) in [
        (CopyPhase::Input, 0),
        (CopyPhase::HashBefore, 0),
        (CopyPhase::Copy, 0),
        (CopyPhase::HashAfter, 16),
        (CopyPhase::MeasureStage, 16),
        (CopyPhase::Complete, 16),
    ] {
        with_native(b"0123456789abcdef", |files, _db, root| {
            let (source, stage, guard) = issued(files, root)?;
            let cancel = WorkCancellation::default();
            let mut fault = FaultIo::new(&cancel);
            fault.cancel_phase = Some(phase);
            let mut result = copy_with_io(
                source,
                stage,
                guard,
                QualificationEnvelope::default(),
                &cancel,
                &mut fault,
            );
            assert!(matches!(result.error, Some(CopyError::Cancelled(actual)) if actual == phase));
            assert!(result.cancellation_requested);
            assert_eq!(
                physical_prefix(&mut result, root)?,
                &b"0123456789abcdef"[..prefix]
            );
            assert!(matches!(result.stage, StageOutcome::Retired(_)));
            assert!(matches!(
                files.try_custody(),
                Err(StorageError::CustodyBusy)
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn prepublication_cancellation_between_read_and_write_or_short_writes_is_observed() -> TestResult {
    for after_read in [true, false] {
        with_native(b"0123456789abcdef", |files, _db, root| {
            let (source, stage, guard) = issued(files, root)?;
            let cancel = WorkCancellation::default();
            let mut fault = FaultIo::new(&cancel);
            fault.cancel_after_read = after_read;
            fault.write_chunk = 3;
            if !after_read {
                fault.cancel_after_write = Some(3);
            }
            let mut result = copy_with_io(
                source,
                stage,
                guard,
                QualificationEnvelope::default(),
                &cancel,
                &mut fault,
            );
            assert!(matches!(
                result.error,
                Some(CopyError::Cancelled(CopyPhase::Copy))
            ));
            assert_eq!(
                physical_prefix(&mut result, root)?,
                if after_read { &b""[..] } else { &b"012"[..] }
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn prepublication_hash_read_error_or_short_eof_produces_no_digest() -> TestResult {
    for error in [true, false] {
        with_native(b"0123456789abcdef", |files, _db, root| {
            let (source, stage, guard) = issued(files, root)?;
            let cancel = WorkCancellation::default();
            let mut fault = FaultIo::new(&cancel);
            fault.hash_error = error;
            if !error {
                fault.hash_eof_after = Some(3);
            }
            let mut result = copy_with_io(
                source,
                stage,
                guard,
                QualificationEnvelope::default(),
                &cancel,
                &mut fault,
            );
            if error {
                assert!(
                    matches!(&result.error, Some(CopyError::Storage { phase: CopyPhase::HashBefore, cause: StorageError::Io { source, .. } }) if source.kind() == io::ErrorKind::PermissionDenied)
                );
            } else {
                assert!(matches!(
                    result.error,
                    Some(CopyError::LengthChanged {
                        expected: 16,
                        observed: 3
                    })
                ));
            }
            assert_eq!(physical_prefix(&mut result, root)?, b"");
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn prepublication_changed_source_or_actual_stage_is_never_measured_success() -> TestResult {
    for change_source in [true, false] {
        with_native(b"0123456789abcdef", |files, _db, root| {
            let (source, stage, guard) = issued(files, root)?;
            let stage_path = fs::read_dir(root.join("staging"))?
                .next()
                .ok_or("successful stage absent")??
                .path();
            let cancel = WorkCancellation::default();
            let mut fault = FaultIo::new(&cancel);
            fault.checkpoint = Some(Box::new(|phase| {
                if (change_source && phase == CopyPhase::HashAfter)
                    || (!change_source && phase == CopyPhase::MeasureStage)
                {
                    // The fault changes actual bytes, not returned observations.
                    let path = if change_source {
                        root.join("archives").join(SOURCE_NAME)
                    } else {
                        stage_path.join("payload")
                    };
                    let changed: &[u8] = if change_source {
                        b"fedcba98765432100"
                    } else {
                        b"fedcba9876543210"
                    };
                    assert!(fs::write(path, changed).is_ok());
                }
            }));
            let mut result = copy_with_io(
                source,
                stage,
                guard,
                QualificationEnvelope::default(),
                &cancel,
                &mut fault,
            );
            if change_source {
                assert!(matches!(
                    result.error,
                    Some(CopyError::Storage {
                        phase: CopyPhase::HashAfter,
                        cause: StorageError::SnapshotChanged
                    })
                ));
                assert_eq!(physical_prefix(&mut result, root)?, b"0123456789abcdef");
                assert_eq!(
                    fs::read(root.join("archives").join(SOURCE_NAME))?,
                    b"fedcba98765432100"
                );
            } else {
                assert!(matches!(result.error, Some(CopyError::DigestChanged)));
                assert_eq!(physical_prefix(&mut result, root)?, b"fedcba9876543210");
                assert_eq!(
                    fs::read(root.join("archives").join(SOURCE_NAME))?,
                    b"0123456789abcdef"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn prepublication_retirement_failure_keeps_actual_stage_source_guard_and_foreign_bytes()
-> TestResult {
    with_native(b"0123456789abcdef", |files, _db, root| {
        let (source, stage, guard) = issued(files, root)?;
        let stage_path = fs::read_dir(root.join("staging"))?
            .next()
            .ok_or("successful stage absent")??
            .path();
        let displaced = root.join("staging/displaced-owned-test");
        let cancel = WorkCancellation::default();
        let mut fault = FaultIo::new(&cancel);
        fault.read_error_after = Some(4);
        fault.checkpoint = Some(Box::new(|phase| {
            if phase == CopyPhase::RetireUnused {
                assert!(fs::rename(&stage_path, &displaced).is_ok());
                assert!(fs::create_dir(&stage_path).is_ok());
                assert!(fs::write(stage_path.join("payload"), b"foreign retained").is_ok());
            }
        }));
        let mut result = copy_with_io(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &cancel,
            &mut fault,
        );
        assert!(matches!(
            &result.error,
            Some(CopyError::Io {
                operation: "read source",
                ..
            })
        ));
        let StageOutcome::RetirementFailed(failure) = &result.stage else {
            return Err("cleanup failure missing".into());
        };
        assert!(matches!(failure.cause, StorageError::SnapshotChanged));
        assert!(!failure.cleanup_uncertain); // checked pre-move failure only
        assert!(failure.trash_name.is_none());
        assert_eq!(physical_prefix(&mut result, root)?, b"0123");
        assert_eq!(fs::read(displaced.join("payload"))?, b"0123");
        assert_eq!(fs::read(stage_path.join("payload"))?, b"foreign retained");
        assert_eq!(
            fs::read(root.join("archives").join(SOURCE_NAME))?,
            b"0123456789abcdef"
        );
        assert_eq!(fs::read_dir(root.join("trash"))?.count(), 0);
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyBusy)
        ));
        Ok(())
    })
}

#[test]
fn prepublication_already_owned_work_can_finish_after_ordinary_intake_stop() -> TestResult {
    with_native(b"0123456789abcdef", |files, database, root| {
        let (source, stage, guard) = issued(files, root)?;
        database.stop_ordinary()?;
        let result = copy_to_stage(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &WorkCancellation::default(),
        );
        assert!(result.error.is_none());
        assert!(matches!(result.stage, StageOutcome::Measured { .. }));
        assert!(matches!(
            files.open_read_source(FileArea::Archives, EntryName::parse(SOURCE_NAME)?),
            Err(StorageError::IntakeClosed)
        ));
        Ok(())
    })
}

#[test]
fn prepublication_checked_counter_overflow_preserves_prior_observation() {
    let mut bytes = u64::MAX;
    assert!(matches!(
        add(&mut bytes, 1),
        Err(CopyError::CounterOverflow)
    ));
    assert_eq!(bytes, u64::MAX);
}

#[test]
fn prepublication_creation_success_delegates_copy_with_exact_bytes_and_source() -> TestResult {
    with_native(b"actual created stage bytes", |files, _database, root| {
        let guard = files.try_custody()?;
        let source =
            files.open_read_source(FileArea::Archives, EntryName::parse(selected_name(root)?)?)?;
        let initial = source.initial();
        let raw_name = source.name().clone();
        let creation = files.create_stage(&guard);
        assert!(creation.is_ok(), "positive creation fixture: {creation:?}");
        let outcome = copy_created_stage(
            source,
            creation,
            guard,
            QualificationEnvelope::default(),
            &WorkCancellation::default(),
        );
        let CreationOutcome::CopyAttempt(mut copied) = outcome else {
            return Err("success did not delegate".into());
        };
        assert!(copied.error.is_none(), "{:?}", copied.error);
        assert_eq!(copied.source.name(), &raw_name);
        assert_eq!(copied.source.revalidate()?, initial);
        assert_eq!(
            physical_prefix(&mut copied, root)?,
            b"actual created stage bytes"
        );
        assert!(matches!(copied.stage, StageOutcome::Measured { .. }));
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyBusy)
        ));
        assert_eq!(
            fs::read(root.join("archives").join(raw_name.as_os_str()))?,
            b"actual created stage bytes"
        );
        Ok(())
    })
}

#[test]
fn prepublication_creation_noallocation_retains_whole_box_prior_stage_and_cancel_truth()
-> TestResult {
    for cancelled in [false, true] {
        with_native(b"original readonly source", |files, database, root| {
            let guard = files.try_custody()?;
            let source = files
                .open_read_source(FileArea::Archives, EntryName::parse(selected_name(root)?)?)?;
            let raw_name = source.name().clone();
            let initial = source.initial();
            // Keep an earlier actual owned allocation. This later NoAllocation
            // call cannot settle, retire or remove it. This is not a prior-
            // uncertainty/fence injection; those cuts remain owner6 evidence.
            let earlier_creation = files.create_stage(&guard);
            assert!(
                earlier_creation.is_ok(),
                "positive prior fixture: {earlier_creation:?}"
            );
            let Ok(mut earlier_stage) = earlier_creation else {
                return Err("prior fixture assertion did not hold".into());
            };
            earlier_stage.write_all(b"earlier unsettled owned bytes")?;
            let before_entries = fs::read_dir(root.join("staging"))?.count();
            database.stop_ordinary()?;
            let creation = files.create_stage(&guard);
            let expected_box = creation
                .as_ref()
                .err()
                .map(|failure| std::ptr::from_ref(failure.as_ref()))
                .ok_or("intake stop did not refuse allocation")?;
            let cancel = WorkCancellation::default();
            if cancelled {
                cancel.cancel();
            }
            // A zero qualification envelope would refuse this source if copy
            // were entered. The real factory cause must survive unchanged.
            let outcome = copy_created_stage(
                source,
                creation,
                guard,
                QualificationEnvelope {
                    source_bytes: Some(0),
                },
                &cancel,
            );
            let CreationOutcome::CreationFailed(mut owned) = outcome else {
                return Err("failed creation entered copy".into());
            };
            assert!(std::ptr::eq(expected_box, owned.failure.as_ref()));
            assert!(matches!(owned.failure.cause(), StorageError::IntakeClosed));
            assert_eq!(
                owned.failure.disposition(),
                StageCreateDisposition::NoAllocation
            );
            assert!(owned.failure.retained_allocation().is_none());
            assert_eq!(owned.cancellation_requested, cancelled);
            assert_eq!(owned.source.name(), &raw_name);
            assert_eq!(owned.source.revalidate()?, initial);
            owned.source.rewind()?;
            let mut actual_source = Vec::new();
            owned.source.read_to_end(&mut actual_source)?;
            assert_eq!(actual_source, b"original readonly source");
            let mut earlier_bytes = Vec::new();
            earlier_stage.inspector().rewind()?;
            earlier_stage.inspector().read_to_end(&mut earlier_bytes)?;
            assert_eq!(earlier_bytes, b"earlier unsettled owned bytes");
            assert_eq!(fs::read_dir(root.join("staging"))?.count(), before_entries);
            assert_eq!(fs::read_dir(root.join("trash"))?.count(), 0);
            assert_eq!(
                fs::read(root.join("archives").join(raw_name.as_os_str()))?,
                b"original readonly source"
            );
            // Close this separate earlier owner while the actual guard remains
            // in owned. Bytes are preserved; no clean finalization is claimed.
            drop(earlier_stage);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn prepublication_creation_success_with_cancel_keeps_existing_retirement_contract() -> TestResult {
    with_native(b"original readonly source", |files, _database, root| {
        let guard = files.try_custody()?;
        let source =
            files.open_read_source(FileArea::Archives, EntryName::parse(selected_name(root)?)?)?;
        let creation = files.create_stage(&guard);
        assert!(creation.is_ok(), "positive creation fixture: {creation:?}");
        let cancel = WorkCancellation::default();
        cancel.cancel();
        let outcome = copy_created_stage(
            source,
            creation,
            guard,
            QualificationEnvelope::default(),
            &cancel,
        );
        let CreationOutcome::CopyAttempt(mut copied) = outcome else {
            return Err("success did not delegate".into());
        };
        assert!(matches!(
            copied.error,
            Some(CopyError::Cancelled(CopyPhase::Input))
        ));
        assert!(copied.cancellation_requested);
        assert_eq!(copied.progress, CopyProgress::default());
        assert_eq!(physical_prefix(&mut copied, root)?, b"");
        assert!(matches!(copied.stage, StageOutcome::Retired(_)));
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyBusy)
        ));
        assert_eq!(
            fs::read(root.join("archives").join(SOURCE_NAME))?,
            b"original readonly source"
        );
        Ok(())
    })
}
