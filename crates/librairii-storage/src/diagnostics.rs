//! Explicit, bounded diagnostics. No free-form strings, errors, paths, tokens,
//! database pages or archives can enter the event API or diagnostic payload.
pub use crate::platform::unix::logging::Logger;
use crate::{
    Result, StorageError,
    database::{Database, WorkCancellation},
    platform::unix::transfer::{Destination, Output},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
mod zip;
pub const MAX_LOG_FILES: usize = 5;
pub const MAX_LOG_BYTES: usize = 256 * 1024;
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Info,
    Warn,
    Error,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    RuntimeStarted,
    RuntimeStopped,
    OperationChanged,
    DiagnosticsExported,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    time: String,
    level: Level,
    event: Event,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
}
impl Entry {
    pub(crate) fn new(
        time: String,
        level: Level,
        event: Event,
        state: Option<librairii_core::OperationState>,
    ) -> Self {
        Self {
            time,
            level,
            event,
            state: state.map(|s| librairii_core::operations::encode_state(s).into()),
        }
    }
}
pub(crate) fn timestamp() -> Result<String> {
    OffsetDateTime::from(std::time::SystemTime::now())
        .format(&Rfc3339)
        .map_err(|_| StorageError::CompatibilityInvariant)
}
pub(crate) fn sanitize(bytes: &[u8]) -> Vec<u8> {
    let mut clean = Vec::new();
    for line in bytes.split(|b| *b == b'\n') {
        if line.len() > 1024 {
            continue;
        }
        let Ok(entry) = serde_json::from_slice::<Entry>(line) else {
            continue;
        };
        if entry.time.len() > 64
            || OffsetDateTime::parse(&entry.time, &Rfc3339).is_err()
            || entry.state.as_deref().is_some_and(|s| {
                !matches!(
                    s,
                    "" | "queued"
                        | "running"
                        | "succeeded"
                        | "partially_succeeded"
                        | "failed"
                        | "cancelled"
                        | "interrupted"
                )
            })
        {
            continue;
        }
        if let Ok(mut encoded) = serde_json::to_vec(&entry) {
            encoded.push(b'\n');
            if clean.len() + encoded.len() <= MAX_LOG_BYTES {
                clean.extend(encoded);
            }
        }
    }
    clean
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format_version: u32,
    generated_at: String,
    operating_system: &'static str,
    architecture: &'static str,
    schema_version: i64,
    story_count: i64,
    shelf_count: i64,
    operation_count: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Report {
    pub file_name: String,
    pub byte_size: u64,
}
#[derive(Debug)]
pub struct ExportFailure {
    pub problem: librairii_core::diagnostics::Problem,
    cause: StorageError,
}
impl ExportFailure {
    pub fn into_storage_error(self) -> StorageError {
        self.cause
    }
}
impl std::fmt::Display for ExportFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.problem.code())
    }
}
impl std::error::Error for ExportFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
#[derive(Debug)]
pub struct Diagnostics<'db, 'logger, 'files, 'library> {
    database: &'db Database<'library>,
    logger: &'logger Logger<'db, 'files, 'library>,
    retained: Vec<Output>,
}
impl<'db, 'logger, 'files, 'library> Diagnostics<'db, 'logger, 'files, 'library> {
    pub fn new(
        database: &'db Database<'library>,
        logger: &'logger Logger<'db, 'files, 'library>,
    ) -> Result<Self> {
        if !logger.matches_database(database) {
            return Err(StorageError::IdentityMismatch);
        }
        Ok(Self {
            database,
            logger,
            retained: Vec::new(),
        })
    }
    pub fn retained_outputs(&self) -> usize {
        self.retained.len()
    }
    /// Only the explicitly selected destination is used; no automatic upload.
    pub fn export(
        &mut self,
        destination: &Path,
        cancel: &WorkCancellation,
    ) -> std::result::Result<Report, ExportFailure> {
        self.export_with(destination, cancel, |output, name| output.publish(name))
            .map_err(|cause| self.failure(cause))
    }
    /// Native save-picker route. The selected leaf is used verbatim; existing
    /// files still refuse publication through the same create-new protocol.
    pub fn export_named(
        &mut self,
        selected: &Path,
        cancel: &WorkCancellation,
    ) -> std::result::Result<Report, ExportFailure> {
        let result = (|| {
            let parent = selected.parent().ok_or(StorageError::SettingsDestination)?;
            let name = selected
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or(StorageError::SettingsDestination)?;
            crate::platform::unix::EntryName::parse(name)?;
            if !name.to_ascii_lowercase().ends_with(".zip") {
                return Err(StorageError::SettingsDestination);
            }
            self.export_choice(parent, Some(name), cancel, |output, name| {
                output.publish(name)
            })
        })();
        result.map_err(|cause| self.failure(cause))
    }
    fn failure(&self, cause: StorageError) -> ExportFailure {
        use librairii_core::diagnostics::Problem;
        let problem = if !self.retained.is_empty() {
            Problem::RecoveryRequired
        } else {
            match &cause {
                StorageError::Export(librairii_core::export::ExportError::Destination) => {
                    Problem::Destination
                }
                StorageError::DestinationCollision => Problem::Collision,
                StorageError::Cancelled => Problem::Cancelled,
                StorageError::DiagnosticLogsUnavailable => Problem::LogsUnavailable,
                StorageError::CustodyRecoveryRequired
                | StorageError::RuntimeFenced
                | StorageError::OwnedTempCleanupFailed { .. } => Problem::RecoveryRequired,
                _ => Problem::ExportFailed,
            }
        };
        ExportFailure { problem, cause }
    }
    fn export_with(
        &mut self,
        destination: &Path,
        cancel: &WorkCancellation,
        publish: impl FnOnce(&mut Output, &str) -> Result<()>,
    ) -> Result<Report> {
        self.export_choice(destination, None, cancel, publish)
    }
    fn export_choice(
        &mut self,
        destination: &Path,
        selected_name: Option<&str>,
        cancel: &WorkCancellation,
        publish: impl FnOnce(&mut Output, &str) -> Result<()>,
    ) -> Result<Report> {
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        if !self.retained.is_empty() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        let destination = Destination::open(destination)
            .map_err(|_| librairii_core::export::ExportError::Destination)?;
        if destination
            .path()
            .starts_with(self.database.library_layout().root())
        {
            return Err(librairii_core::export::ExportError::Destination.into());
        }
        let generated = timestamp()?;
        let at = generated.clone();
        let (manifest, identifier) = self
            .database
            .read(move |tx| {
                let manifest = Manifest {
                    format_version: 1,
                    generated_at: at,
                    operating_system: if cfg!(target_os = "macos") {
                        "darwin"
                    } else {
                        "linux"
                    },
                    architecture: if cfg!(target_arch = "aarch64") {
                        "arm64"
                    } else {
                        "amd64"
                    },
                    schema_version: tx.query_row(
                        "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
                        [],
                        |r| r.get(0),
                    )?,
                    story_count: tx.query_row("SELECT count(*) FROM stories", [], |r| r.get(0))?,
                    shelf_count: tx.query_row("SELECT count(*) FROM shelves", [], |r| r.get(0))?,
                    operation_count: tx.query_row(
                        "SELECT count(*) FROM file_operations",
                        [],
                        |r| r.get(0),
                    )?,
                };
                Ok((
                    manifest,
                    tx.query_row("SELECT lower(hex(randomblob(8)))", [], |r| {
                        r.get::<_, String>(0)
                    })?,
                ))
            })?
            .wait()?;
        let mut entries = vec![(
            "diagnostics.json".to_owned(),
            serde_json::to_vec(&manifest).map_err(|_| StorageError::CompatibilityInvariant)?,
        )];
        for (name, bytes) in self
            .logger
            .snapshots()
            .map_err(|_| StorageError::DiagnosticLogsUnavailable)?
        {
            entries.push((format!("logs/{name}"), bytes));
        }
        let bytes = zip::encode(&entries)?;
        let hash = librairii_core::Checksum::from_bytes(Sha256::digest(&bytes).into());
        let name = selected_name.map(str::to_owned).unwrap_or_else(|| {
            format!(
                "librairii-diagnostics-{}-{identifier}.zip",
                generated.replace(['-', ':'], "")
            )
        });
        let mut output = match destination.create_output() {
            Ok(output) => output,
            Err(failure) => {
                let failure = *failure;
                if let Some(output) = failure.output {
                    self.retain(output);
                }
                return Err(failure.cause);
            }
        };
        let result = (|| {
            output.stream(
                &mut bytes.as_slice(),
                hash,
                bytes.len() as u64,
                cancel,
                |_| {},
            )?;
            destination.revalidate()?;
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            publish(&mut output, &name)
        })();
        if let Err(cause) = result {
            if output.published || output.uncertain {
                self.retain(output);
                return Err(cause);
            }
            if let Err(cleanup) = output.cleanup() {
                self.retain(output);
                return Err(StorageError::OwnedTempCleanupFailed {
                    work: Box::new(cause),
                    cleanup: Box::new(cleanup),
                });
            }
            return Err(cause);
        }
        // Export has committed. A diagnostic-log failure cannot change that fact.
        let _ = self.logger.record(
            Level::Info,
            Event::DiagnosticsExported,
            Some(librairii_core::OperationState::Succeeded),
        );
        Ok(Report {
            file_name: name,
            byte_size: bytes.len() as u64,
        })
    }
    fn retain(&mut self, output: Output) {
        if output.retention_required() {
            self.database.fence_external_effect();
            self.retained.push(output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{native::NativeLibrary, profile::NativeProfileStore, test_support::native_copy};
    use std::{error::Error, fs, process::Command};
    type TestResult = std::result::Result<(), Box<dyn Error>>;
    const VALID:&[u8]=b"{\"time\":\"2026-10-22T00:00:00Z\",\"level\":\"info\",\"event\":\"operation_changed\",\"state\":\"succeeded\"}\n";
    #[test]
    fn native_diagnostics_rotation_keeps_five_bounded_owned_logs() -> TestResult {
        let (_roots, copy, locations) = native_copy()?;
        let root = copy.root().to_owned();
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                .map_err(|e| e.to_string())?;
        let mut db = Database::open_native(&native)?;
        let mut full = VALID.repeat(MAX_LOG_BYTES / VALID.len());
        full.resize(MAX_LOG_BYTES, b'\n');
        for i in 0..MAX_LOG_FILES {
            let name = if i == 0 {
                "events.jsonl".into()
            } else {
                format!("events.{i}.jsonl")
            };
            fs::write(root.join("logs").join(name), &full)?;
        }
        {
            let files = crate::platform::unix::LibraryFiles::open_native(&db)?;
            let logger = Logger::new(&db, &files)?;
            assert!(Logger::new(&db, &files).is_err());
            // Fixture fills the retained current object; production records perform
            // rotation. Avoid thousands of fsyncs just to reach each boundary.
            for _ in 0..6 {
                fs::write(root.join("logs/events.jsonl"), &full)?;
                logger.record(
                    Level::Info,
                    Event::OperationChanged,
                    Some(librairii_core::OperationState::Succeeded),
                )?;
                let names =
                    fs::read_dir(root.join("logs"))?.collect::<std::result::Result<Vec<_>, _>>()?;
                assert_eq!(names.len(), MAX_LOG_FILES);
                for file in names {
                    assert!(file.metadata()?.len() <= MAX_LOG_BYTES as u64);
                }
                assert!(
                    logger
                        .snapshots()?
                        .iter()
                        .all(|(_, b)| b.len() <= MAX_LOG_BYTES)
                );
            }
            logger.flush()?;
        }
        db.close()?;
        drop(db);
        native.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[test]
    fn native_diagnostics_zip_has_only_sanitized_logs_and_aggregate_manifest() -> TestResult {
        let (roots, copy, locations) = native_copy()?;
        let root = copy.root().to_owned();
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                .map_err(|e| e.to_string())?;
        let mut db = Database::open_native(&native)?;
        let marker = "PRIVATE_SENTINEL_TOKEN";
        let mut payload = VALID.to_vec();
        for bad in [
            serde_json::json!({"time":"2026-10-22T00:00:00Z","level":"info","event":"runtime_started","token":marker,"path":root}),
            serde_json::json!({"time":"2026-10-22T00:00:00Z","level":"info","event":"operation_changed","state":marker}),
            serde_json::json!({"time":"2026-10-22T00:00:00Z","level":"private","event":"runtime_started"}),
            serde_json::json!({"time":marker,"level":"info","event":"runtime_started"}),
        ] {
            payload.extend(serde_json::to_vec(&bad)?);
            payload.push(b'\n');
        }
        payload.extend_from_slice(b"{\"time\":\"2026-10-22T00:00:00Z\",\"level\":\"info\",\"event\":\"runtime_started\"} trailing\n");
        fs::write(root.join("logs/events.jsonl"), &payload)?;
        fs::write(root.join("logs/personal-token.txt"), marker)?;
        let destination = roots.profile().join("diagnostics");
        fs::create_dir(&destination)?;
        {
            let files = crate::platform::unix::LibraryFiles::open_native(&db)?;
            let logger = Logger::new(&db, &files)?;
            logger.record(Level::Info, Event::RuntimeStarted, None)?;
            let before = fs::read(root.join("logs/events.jsonl"))?;
            let mut diagnostics = Diagnostics::new(&db, &logger)?;
            let report = diagnostics.export(&destination, &Default::default())?;
            assert_eq!(diagnostics.retained_outputs(), 0);
            let path = destination.join(&report.file_name);
            assert_eq!(fs::metadata(&path)?.len(), report.byte_size);
            // Independent ZIP implementation checks CRC, central directory, modes,
            // filenames and exact allowed manifest/event keys.
            let result=Command::new("python3").args(["-I","-c",r#"
import json,sys,zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
 assert z.namelist()==['diagnostics.json','logs/events.jsonl']
 assert z.testzip() is None and not z.comment
 m=json.loads(z.read('diagnostics.json'))
 assert set(m)=={'formatVersion','generatedAt','operatingSystem','architecture','schemaVersion','storyCount','shelfCount','operationCount'}
 assert m['formatVersion']==1 and m['schemaVersion']==18
 assert m['operatingSystem'] in ('darwin','linux') and m['architecture'] in ('amd64','arm64')
 for i in z.infolist():
  assert i.compress_type==0 and i.external_attr>>16==0o100600 and not i.comment
  b=z.read(i)
  assert b'PRIVATE_SENTINEL_TOKEN' not in b and sys.argv[2].encode() not in b and b'.sqlite' not in b
 rows=[json.loads(line) for line in z.read('logs/events.jsonl').splitlines()]
 assert len(rows)==2
 assert all(set(r)<= {'time','level','event','state'} for r in rows)
 assert rows[0]['event']=='operation_changed' and rows[1]['event']=='runtime_started'
"#]).arg(&path).arg(&root).output()?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(fs::read_dir(&destination)?.count(), 1);
            assert!(fs::read(root.join("logs/events.jsonl"))?.starts_with(&before));
            assert_eq!(
                fs::read(root.join("logs/personal-token.txt"))?,
                marker.as_bytes()
            );
            logger.flush()?;
        }
        db.close()?;
        drop(db);
        native.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[test]
    fn native_diagnostics_requires_explicit_external_selection_and_localizes_safe_failures()
    -> TestResult {
        use librairii_core::diagnostics::Problem;
        let (roots, copy, locations) = native_copy()?;
        let root = copy.root().to_owned();
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                .map_err(|e| e.to_string())?;
        let mut db = Database::open_native(&native)?;
        let destination = roots.profile().join("diagnostics");
        fs::create_dir(&destination)?;
        let token = roots.profile().join("personal-token.txt");
        fs::write(&token, b"PRIVATE_TOKEN_NEVER_READ")?;
        std::os::unix::fs::symlink(&token, root.join("logs/events.jsonl"))?;
        {
            let files = crate::platform::unix::LibraryFiles::open_native(&db)?;
            let logger = Logger::new(&db, &files)?;
            let mut diagnostics = Diagnostics::new(&db, &logger)?;
            for selected in [
                Path::new("relative"),
                root.as_path(),
                &root.join("archives"),
            ] {
                let error = diagnostics
                    .export(selected, &Default::default())
                    .err()
                    .ok_or("unsafe destination accepted")?;
                assert_eq!(error.problem, Problem::Destination);
                assert_eq!(error.to_string(), "diagnostics.destination_invalid");
            }
            let cancel = WorkCancellation::default();
            cancel.cancel();
            assert_eq!(
                diagnostics
                    .export(&destination, &cancel)
                    .err()
                    .ok_or("cancellation ignored")?
                    .problem,
                Problem::Cancelled
            );
            let error = diagnostics
                .export(&destination, &Default::default())
                .err()
                .ok_or("symlink log was read")?;
            assert_eq!(error.problem, Problem::LogsUnavailable);
            for problem in [
                Problem::Destination,
                Problem::Collision,
                Problem::Cancelled,
                Problem::LogsUnavailable,
                Problem::ExportFailed,
                Problem::RecoveryRequired,
            ] {
                assert_ne!(problem.message("en"), problem.message("fr-FR"));
                assert!(problem.code().starts_with("diagnostics."));
                for locale in ["en", "fr-FR"] {
                    assert!(
                        !problem
                            .message(locale)
                            .contains(root.to_str().ok_or("fixture UTF8")?)
                    );
                    assert!(!problem.message(locale).contains("PRIVATE_TOKEN"));
                }
            }
            assert_eq!(diagnostics.retained_outputs(), 0);
            assert_eq!(fs::read_dir(&destination)?.count(), 0);
            assert_eq!(fs::read(&token)?, b"PRIVATE_TOKEN_NEVER_READ");
        }
        // A read-only refusal before file acquisition does not claim uncertain
        // library effects or prevent ordinary clean closure.
        db.close()?;
        drop(db);
        native.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[test]
    fn native_diagnostics_no_clobber_cancel_and_uncertain_publication_retain_actual_output()
    -> TestResult {
        for mode in ["race", "cancel", "uncertain"] {
            let (roots, copy, locations) = native_copy()?;
            let profile = NativeProfileStore::create_fresh(&locations.native)?;
            let native =
                NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                    .map_err(|e| e.to_string())?;
            let mut db = Database::open_native(&native)?;
            let destination = roots.profile().join("diagnostics");
            fs::create_dir(&destination)?;
            {
                let files = crate::platform::unix::LibraryFiles::open_native(&db)?;
                let logger = Logger::new(&db, &files)?;
                logger.record(Level::Info, Event::RuntimeStarted, None)?;
                let mut diagnostics = Diagnostics::new(&db, &logger)?;
                let cancel = WorkCancellation::default();
                let mut chosen = None;
                let result = diagnostics.export_with(&destination, &cancel, |output, name| {
                    chosen = Some(name.to_owned());
                    match mode {
                        "race" => {
                            fs::write(destination.join(name), b"foreign destination")
                                .map_err(|e| StorageError::io("inject raced diagnostic", e))?;
                            output.publish(name)
                        }
                        "cancel" => {
                            cancel.cancel();
                            Err(StorageError::Cancelled)
                        }
                        _ => output.publish_with_failed_flush(name),
                    }
                });
                assert!(result.is_err());
                assert_eq!(
                    diagnostics.retained_outputs(),
                    usize::from(mode == "uncertain")
                );
                let chosen = chosen.ok_or("publication checkpoint")?;
                if mode == "race" {
                    assert_eq!(fs::read(destination.join(&chosen))?, b"foreign destination");
                }
                if mode == "cancel" {
                    assert_eq!(fs::read_dir(&destination)?.count(), 0);
                }
                if mode == "uncertain" {
                    assert!(destination.join(&chosen).is_file());
                }
                if mode != "uncertain" {
                    logger.flush()?;
                }
            }
            if mode == "uncertain" {
                assert!(db.close().is_err());
                drop(db);
                assert!(native.finish().is_err());
            } else {
                db.close()?;
                drop(db);
                native.finish().map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
