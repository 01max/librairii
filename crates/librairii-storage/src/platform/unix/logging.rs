//! The five fixed log names are operated on only through retained regular
//! files and the actual library-owned logs directory. Failed rotations fence.
use super::*;
use crate::diagnostics::{Entry, Event, Level, MAX_LOG_BYTES, MAX_LOG_FILES, sanitize, timestamp};
use std::sync::Mutex;
#[derive(Debug)]
struct LogFile {
    file: File,
    identity: Option<FileIdentity>,
}
#[derive(Debug)]
struct Inner {
    logs: [Option<LogFile>; MAX_LOG_FILES],
    retired: Option<LogFile>,
    initialized: bool,
    fenced: bool,
    uncertain: bool,
}
#[derive(Debug)]
pub struct Logger<'db, 'files, 'library> {
    database: &'db Database<'library>,
    files: &'files LibraryFiles<'library>,
    directory: Directory,
    inner: Mutex<Inner>,
}
fn name(index: usize) -> OsString {
    if index == 0 {
        "events.jsonl".into()
    } else {
        format!("events.{index}.jsonl").into()
    }
}
impl<'db, 'files, 'library> Logger<'db, 'files, 'library> {
    pub fn new(
        database: &'db Database<'library>,
        files: &'files LibraryFiles<'library>,
    ) -> Result<Self> {
        files.matches_database(database)?;
        files.revalidate()?;
        let directory = files.root.child(OsStr::new("logs"))?;
        if database
            .logger_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(StorageError::RuntimeInUse);
        }
        Ok(Self {
            database,
            files,
            directory,
            inner: Mutex::new(Inner {
                logs: std::array::from_fn(|_| None),
                retired: None,
                initialized: false,
                fenced: false,
                uncertain: false,
            }),
        })
    }
    fn validate(&self, inner: &Inner) -> Result<()> {
        self.files.drain_ownership()?;
        self.files
            .root
            .require_name(OsStr::new("logs"), self.directory.identity)?;
        if inner.fenced {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        for (index, file) in inner.logs.iter().enumerate() {
            match file {
                Some(file) => {
                    require_regular(&file.file)?;
                    self.directory.require_name(
                        &name(index),
                        file.identity.ok_or(StorageError::CustodyRecoveryRequired)?,
                    )?;
                    if file
                        .file
                        .metadata()
                        .map_err(|e| StorageError::io("measure event log", e))?
                        .len()
                        > MAX_LOG_BYTES as u64
                    {
                        return Err(StorageError::HashSizeLimit);
                    }
                }
                None => match native::statat(
                    &self.directory.file,
                    name(index),
                    AtFlags::SYMLINK_NOFOLLOW,
                ) {
                    Err(rustix::io::Errno::NOENT) => {}
                    Ok(_) => return Err(StorageError::SnapshotChanged),
                    Err(e) => return Err(StorageError::io("observe absent log slot", e.into())),
                },
            }
        }
        Ok(())
    }
    fn initialize(&self, inner: &mut Inner) -> Result<()> {
        if !inner.initialized {
            self.files.revalidate()?;
            for index in 0..MAX_LOG_FILES {
                if let Some(file) = self.directory.read_regular(&name(index))? {
                    inner.logs[index] = Some(LogFile {
                        identity: Some(FileIdentity::from_file(&file)?),
                        file,
                    });
                }
            }
            inner.initialized = true;
            self.validate(inner)?;
            // Reopen current for append, retaining its observed name identity.
            if let Some(current) = &mut inner.logs[0] {
                let file = self
                    .directory
                    .open_regular(&name(0), OFlags::RDWR | OFlags::APPEND)?;
                if FileIdentity::from_file(&file)?
                    != current
                        .identity
                        .ok_or(StorageError::CustodyRecoveryRequired)?
                {
                    return Err(StorageError::SnapshotChanged);
                }
                current.file = file;
            }
        }
        self.validate(inner)
    }
    fn create_current(&self, inner: &mut Inner) -> Result<()> {
        if inner.logs[0].is_none() {
            let file: File = native::openat(
                &self.directory.file,
                name(0),
                OFlags::RDWR
                    | OFlags::APPEND
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|e| StorageError::io("create event log", e.into()))?
            .into();
            inner.logs[0] = Some(LogFile {
                identity: None,
                file,
            });
            inner.uncertain = true;
            let current = inner.logs[0]
                .as_mut()
                .ok_or(StorageError::CompatibilityInvariant)?;
            current.identity = Some(FileIdentity::from_file(&current.file)?);
            self.validate(inner)?;
            self.directory.flush()?;
        }
        Ok(())
    }
    fn rotate(&self, inner: &mut Inner) -> Result<()> {
        self.validate(inner)?;
        inner.uncertain = true;
        if let Some(oldest) = &inner.logs[MAX_LOG_FILES - 1] {
            self.directory.require_name(
                &name(MAX_LOG_FILES - 1),
                oldest
                    .identity
                    .ok_or(StorageError::CustodyRecoveryRequired)?,
            )?;
            native::unlinkat(
                &self.directory.file,
                name(MAX_LOG_FILES - 1),
                AtFlags::empty(),
            )
            .map_err(|e| StorageError::io("retire oldest event log", e.into()))?;
            inner.retired = inner.logs[MAX_LOG_FILES - 1].take();
            self.directory.flush()?;
            inner.retired = None;
        }
        for index in (0..MAX_LOG_FILES - 1).rev() {
            if let Some(file) = &inner.logs[index] {
                durable_file_flush(&file.file)?;
                self.directory.require_name(
                    &name(index),
                    file.identity.ok_or(StorageError::CustodyRecoveryRequired)?,
                )?;
                native::renameat_with(
                    &self.directory.file,
                    name(index),
                    &self.directory.file,
                    name(index + 1),
                    RenameFlags::NOREPLACE,
                )
                .map_err(|e| StorageError::io("rotate retained event log", e.into()))?;
                inner.logs[index + 1] = inner.logs[index].take();
                self.directory.flush()?;
            }
        }
        self.create_current(inner)
    }
    pub fn record(
        &self,
        level: Level,
        event: Event,
        state: Option<librairii_core::OperationState>,
    ) -> Result<()> {
        let entry = Entry::new(timestamp()?, level, event, state);
        let mut payload =
            serde_json::to_vec(&entry).map_err(|_| StorageError::CompatibilityInvariant)?;
        payload.push(b'\n');
        self.with_inner(|inner| {
            self.initialize(inner)?;
            self.create_current(inner)?;
            let current = inner.logs[0]
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            if current
                .file
                .metadata()
                .map_err(|e| StorageError::io("measure current event log", e))?
                .len()
                + payload.len() as u64
                > MAX_LOG_BYTES as u64
            {
                self.rotate(inner)?;
            }
            let current = inner.logs[0]
                .as_mut()
                .ok_or(StorageError::CompatibilityInvariant)?;
            inner.uncertain = true;
            current
                .file
                .write_all(&payload)
                .map_err(|e| StorageError::io("append event log", e))?;
            durable_file_flush(&current.file)?;
            self.validate(inner)
        })
    }
    pub(crate) fn snapshots(&self) -> Result<Vec<(String, Vec<u8>)>> {
        self.with_inner(|inner| {
            self.initialize(inner)?;
            let mut snapshots = Vec::with_capacity(MAX_LOG_FILES);
            for (index, file) in inner.logs.iter_mut().enumerate() {
                let Some(file) = file else {
                    continue;
                };
                durable_file_flush(&file.file)?;
                file.file
                    .rewind()
                    .map_err(|e| StorageError::io("rewind diagnostic log snapshot", e))?;
                let mut bytes = Vec::new();
                Read::by_ref(&mut file.file)
                    .take(MAX_LOG_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| StorageError::io("read bounded event log", e))?;
                if bytes.len() > MAX_LOG_BYTES {
                    return Err(StorageError::HashSizeLimit);
                }
                let clean = sanitize(&bytes);
                if !clean.is_empty() {
                    snapshots.push((
                        name(index)
                            .to_str()
                            .ok_or(StorageError::CompatibilityInvariant)?
                            .into(),
                        clean,
                    ));
                }
            }
            self.validate(inner)?;
            Ok(snapshots)
        })
    }
    pub fn flush(&self) -> Result<()> {
        self.with_inner(|inner| {
            self.initialize(inner)?;
            for file in inner.logs.iter().flatten() {
                durable_file_flush(&file.file)?;
            }
            self.directory.flush()
        })
    }
    fn with_inner<T>(&self, work: impl FnOnce(&mut Inner) -> Result<T>) -> Result<T> {
        let mut inner = self.inner.lock().map_err(|_| StorageError::RuntimeFenced)?;
        let result = work(&mut inner);
        if result.is_err() {
            inner.fenced = true;
            if inner.uncertain {
                self.database.fence_external_effect();
            }
        } else {
            inner.uncertain = false;
        }
        result
    }
    pub(crate) fn matches_database(&self, database: &Database<'_>) -> bool {
        std::ptr::eq(self.database, database)
    }
}
impl Drop for Logger<'_, '_, '_> {
    fn drop(&mut self) {
        if self.inner.is_poisoned() {
            self.database.fence_external_effect();
        }
        self.database.logger_active.store(false, Ordering::Release);
    }
}
