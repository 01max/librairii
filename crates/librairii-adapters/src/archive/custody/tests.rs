use super::*;
use librairii_storage::{
    adoption::{ClosedLibraryInput, ClosedManifest, IndependentCopy},
    database::Database,
    native::NativeLibrary,
    platform::unix::{EntryName, FileArea, LibraryFiles},
    profile::{NativeProfileStore, ProfileDocument, ProfileLocations},
};
use librairii_testkit::TestRoots;
use std::{error::Error, ffi::OsStr, fs, path::Path};

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
        quoted(&format!("archives/{SOURCE_NAME}")),
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
) -> librairii_storage::Result<(
    OwnedReadSource<'files, 'library>,
    OwnedStage<'files, 'library>,
    Custody<'library>,
)> {
    let guard = files.try_custody()?;
    let source = files.open_read_source(FileArea::Archives, EntryName::parse(SOURCE_NAME)?)?;
    // Only the actually successful constructor enters the mechanism proof.
    let stage = files.create_stage(&guard)?;
    Ok((source, stage, guard))
}

#[test]
fn prepublication_copy_retains_real_native_owners_raw_name_and_exact_eof_bytes() -> TestResult {
    let bytes: Vec<u8> = (0..(HASH_BUFFER_BYTES * 2 + 37))
        .map(|i| (i % 251) as u8)
        .collect();
    with_native(&bytes, |files, _db, root| {
        let (source, stage, guard) = issued(files)?;
        let initial = source.initial();
        let mut result = copy_to_stage(
            source,
            stage,
            guard,
            QualificationEnvelope::default(),
            &WorkCancellation::default(),
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.source.name().as_os_str(), OsStr::new(SOURCE_NAME));
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
    with_native(b"", |files, _db, _root| {
        let (source, stage, guard) = issued(files)?;
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
    with_native(b"abcdef", |files, _db, _root| {
        let (mut source, mut stage, guard) = issued(files)?;
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
    with_native(b"new source", |files, _db, _root| {
        let (source, mut stage, guard) = issued(files)?;
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
