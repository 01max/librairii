//! Descriptor-relative mechanisms for actual development/native library owners.
//!
//! No public constructor accepts an arbitrary path as mutation authority. All
//! claims through this module share the library's custody exclusion. Neither a
//! checksum nor a pathname can construct an owned file receipt. There is no
//! destructive Drop, ambient-path publication or rename-with-replacement here.
//!
//! The private namespace and its single owner exclude competing product writes
//! to stage entries. This is not a sandbox against hostile same-UID processes:
//! witnesses are checked before/after effects, and detected substitution or a
//! lost durability acknowledgement retains bytes and fences new claims. It is
//! never reported as proven-not-published. An unresolved failure cannot be
//! cleared by dropping a guard, returning an error or enqueueing a DTO.
use super::{StreamDigest, hash_stream};
mod export_output;
pub(crate) mod logging;
pub(crate) mod removal;
pub(crate) mod transfer;
use crate::{
    IsolatedLibrary, Result, StorageError,
    database::{Database, WorkCancellation},
    native::NativeLibrary,
};
use rustix::fs::{self as native, AtFlags, Mode, OFlags, RenameFlags};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::MetadataExt,
    },
    path::{Component, Path},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

const PAYLOAD: &str = "payload";
mod import_source;
pub(crate) use import_source::CopyPoint;
pub use import_source::PickedSource;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

/// A lossless single native component. Validation does not confer ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryName(OsString);
impl EntryName {
    pub fn parse(name: impl Into<OsString>) -> Result<Self> {
        let name = name.into();
        let mut parts = Path::new(&name).components();
        if name.as_bytes().contains(&0)
            || name.as_bytes().contains(&b'/')
            || !matches!(parts.next(), Some(Component::Normal(_)))
            || parts.next().is_some()
        {
            return Err(StorageError::InvalidManagedPath);
        }
        Ok(Self(name))
    }
    pub fn as_os_str(&self) -> &OsStr {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    device: u64,
    inode: u64,
}
impl FileIdentity {
    pub(crate) fn from_file(file: &File) -> Result<Self> {
        let stat = file
            .metadata()
            .map_err(|e| StorageError::io("inspect retained file", e))?;
        Ok(Self {
            device: stat.dev(),
            inode: stat.ino(),
        })
    }
    fn from_stat(stat: &native::Stat) -> Self {
        Self {
            #[cfg(target_os = "macos")]
            device: stat.st_dev as u64,
            #[cfg(target_os = "linux")]
            device: stat.st_dev,
            inode: stat.st_ino,
        }
    }
}

/// A retained directory object, not a canonicalized path used again for I/O.
#[derive(Debug)]
pub(crate) struct Directory {
    file: File,
    identity: FileIdentity,
}
impl Directory {
    pub(crate) fn writable_control(&self, observed: ConfigTemp) -> Result<ConfigTemp> {
        let file = self.open_regular(&observed.name, OFlags::RDWR)?;
        if FileIdentity::from_file(&file)? != observed.identity {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(ConfigTemp {
            file,
            name: observed.name,
            identity: observed.identity,
        })
    }
    pub(crate) fn available_bytes(&self) -> Result<Option<u64>> {
        let stat = native::fstatvfs(&self.file)
            .map_err(|e| StorageError::io("observe destination free space", e.into()))?;
        Ok(if stat.f_frsize == 0 {
            None
        } else {
            stat.f_bavail.checked_mul(stat.f_frsize)
        })
    }
    pub(crate) fn recover_control(&self, allocation: &ControlAllocation) -> Result<ConfigTemp> {
        let file = allocation
            .file
            .as_ref()
            .ok_or(StorageError::RecoveryBindingRequired)?;
        require_regular(file)?;
        let identity = FileIdentity::from_file(file)?;
        self.require_name(&allocation.name, identity)?;
        Ok(ConfigTemp {
            file: file
                .try_clone()
                .map_err(|e| StorageError::io("retain incomplete owned control", e))?,
            name: allocation.name.clone(),
            identity,
        })
    }
    pub(crate) fn remove_file_matching(&self, name: &OsStr, file: &File) -> Result<()> {
        require_regular(file)?;
        self.require_name(name, FileIdentity::from_file(file)?)?;
        native::unlinkat(&self.file, name, AtFlags::empty())
            .map_err(|e| StorageError::io("retire verified owned file", e.into()))?;
        self.flush()
    }
    pub(crate) fn remove_empty_child(&self, name: &OsStr, child: &Self) -> Result<()> {
        child.entries(0, &WorkCancellation::default())?;
        self.require_name(name, child.identity)?;
        native::unlinkat(&self.file, name, AtFlags::REMOVEDIR)
            .map_err(|e| StorageError::io("retire verified empty directory", e.into()))?;
        self.flush()
    }
    pub(crate) fn publish_directory(
        &self,
        source: &OsStr,
        child: &Self,
        destination: &OsStr,
    ) -> Result<()> {
        EntryName::parse(source)?;
        EntryName::parse(destination)?;
        self.require_name(source, child.identity)?;
        native::renameat_with(
            &self.file,
            source,
            &self.file,
            destination,
            native::RenameFlags::NOREPLACE,
        )
        .map_err(|e| StorageError::io("publish directory without replacement", e.into()))?;
        self.require_name(destination, child.identity)?;
        self.flush()
    }
    pub(crate) fn metadata(&self) -> Result<std::fs::Metadata> {
        self.file
            .metadata()
            .map_err(|e| StorageError::io("observe anchored directory metadata", e))
    }
    pub(crate) fn identity(&self) -> FileIdentity {
        self.identity
    }
    pub(crate) fn duplicate(&self) -> Result<Self> {
        Self::from_file(
            self.file
                .try_clone()
                .map_err(|e| StorageError::io("retain directory object", e))?,
        )
    }
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let file: File = native::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| StorageError::io("anchor owned directory", e.into()))?
        .into();
        Self::from_file(file)
    }
    fn from_file(file: File) -> Result<Self> {
        if !file
            .metadata()
            .map_err(|e| StorageError::io("inspect anchored directory", e))?
            .is_dir()
        {
            return Err(StorageError::PathEscape);
        }
        let identity = FileIdentity::from_file(&file)?;
        Ok(Self { file, identity })
    }
    pub(crate) fn child(&self, name: &OsStr) -> Result<Self> {
        let file = native::openat(
            &self.file,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| StorageError::io("anchor owned child directory", e.into()))?;
        let child = Self::from_file(file.into())?;
        self.require_name(name, child.identity)?;
        Ok(child)
    }
    pub(crate) fn create_child(&self, name: &OsStr) -> Result<Self> {
        EntryName::parse(name)?;
        native::mkdirat(&self.file, name, Mode::RWXU)
            .map_err(|e| StorageError::io("create fresh anchored directory", e.into()))?;
        self.child(name)
    }
    fn require_name(&self, name: &OsStr, identity: FileIdentity) -> Result<()> {
        let stat = native::statat(&self.file, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|e| StorageError::io("revalidate owned entry", e.into()))?;
        if FileIdentity::from_stat(&stat) != identity {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(())
    }
    pub(crate) fn open_regular(&self, name: &OsStr, flags: OFlags) -> Result<File> {
        // NONBLOCK prevents an attacker-substituted FIFO from blocking before
        // fstat can reject it. NOFOLLOW applies to this final single component.
        let file: File = native::openat(
            &self.file,
            name,
            flags | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|e| StorageError::io("open owned regular file", e.into()))?
        .into();
        require_regular(&file)?;
        self.require_name(name, FileIdentity::from_file(&file)?)?;
        Ok(file)
    }
    pub(crate) fn flush(&self) -> Result<()> {
        native::fsync(&self.file)
            .map_err(|e| StorageError::io("flush anchored directory", e.into()))?;
        #[cfg(target_os = "macos")]
        native::fcntl_fullfsync(&self.file)
            .map_err(|e| StorageError::io("flush directory metadata to device", e.into()))?;
        Ok(())
    }
    pub(crate) fn matches_metadata(&self, metadata: &std::fs::Metadata) -> Result<()> {
        if !metadata.is_dir()
            || self.identity
                != (FileIdentity {
                    device: metadata.dev(),
                    inode: metadata.ino(),
                })
        {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(())
    }
    pub(crate) fn read_regular(&self, name: &OsStr) -> Result<Option<File>> {
        match native::statat(&self.file, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
            Err(error) => Err(StorageError::io(
                "inspect owned readable entry",
                error.into(),
            )),
            Ok(_) => self.open_regular(name, OFlags::RDONLY).map(Some),
        }
    }

    /// Enumerate this retained object with an independent directory cursor.
    /// One unexpected entry is sufficient to refuse a closed-manifest proof;
    /// input names remain lossless and never become cleanup authority.
    pub(crate) fn entries(
        &self,
        limit: usize,
        cancellation: &WorkCancellation,
    ) -> Result<Vec<OsString>> {
        let mut entries = Vec::new();
        let directory = native::Dir::read_from(&self.file)
            .map_err(|e| StorageError::io("enumerate anchored directory", e.into()))?;
        for entry in directory {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let entry =
                entry.map_err(|e| StorageError::io("read anchored directory entry", e.into()))?;
            let bytes = entry.file_name().to_bytes();
            if matches!(bytes, b"." | b"..") {
                continue;
            }
            if entries.len() >= limit {
                return Err(StorageError::AdoptionInputChanged);
            }
            entries.push(OsString::from_vec(bytes.to_vec()));
        }
        entries.sort();
        Ok(entries)
    }
}

/// Private replace-owned-control capability. The profile and native state
/// protocols invoke it under their actual namespace/root leases and owned state. It has no
/// archive/custody publication API and accepts no arbitrary destination path.
#[derive(Debug)]
pub(crate) struct ConfigTemp {
    pub(crate) file: File,
    pub(crate) name: OsString,
    identity: FileIdentity,
}

#[derive(Debug)]
pub(crate) struct ConfigReplaceFailure {
    pub(crate) temporary: ConfigTemp,
    pub(crate) changed: bool,
    pub(crate) cause: StorageError,
}

#[derive(Debug)]
pub(crate) struct ControlAllocation {
    pub(crate) file: Option<File>,
    pub(crate) name: OsString,
}
#[derive(Debug)]
pub(crate) struct ControlCreateFailure {
    pub(crate) cause: StorageError,
    pub(crate) allocation: ControlAllocation,
}
impl std::fmt::Display for ControlCreateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "control creation {:?}: {}",
            self.allocation.name, self.cause
        )
    }
}
impl std::error::Error for ControlCreateFailure {}

impl Directory {
    pub(crate) fn create_control_record(
        &self,
        name: &OsStr,
    ) -> std::result::Result<ConfigTemp, Box<ControlCreateFailure>> {
        self.create_control_with(name, |_| Ok(()))
    }
    fn create_control_with(
        &self,
        name: &OsStr,
        after_open: impl FnOnce(&File) -> Result<()>,
    ) -> std::result::Result<ConfigTemp, Box<ControlCreateFailure>> {
        let mut allocation = ControlAllocation {
            file: None,
            name: name.to_os_string(),
        };
        let result = (|| {
            EntryName::parse(name)?;
            let fd = native::openat(
                &self.file,
                name,
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|e| StorageError::io("create owned control file", e.into()))?;
            allocation.file = Some(File::from(fd));
            let file = allocation
                .file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            after_open(file)?;
            require_regular(file)?;
            let identity = FileIdentity::from_file(file)?;
            self.require_name(name, identity)?;
            Ok(identity)
        })();
        match result {
            Ok(identity) => match allocation.file.take() {
                Some(file) => Ok(ConfigTemp {
                    file,
                    name: allocation.name,
                    identity,
                }),
                None => Err(Box::new(ControlCreateFailure {
                    cause: StorageError::CompatibilityInvariant,
                    allocation,
                })),
            },
            Err(cause) => Err(Box::new(ControlCreateFailure { cause, allocation })),
        }
    }
    pub(crate) fn read_config(&self, name: &OsStr) -> Result<ConfigTemp> {
        let file = self.open_regular(name, OFlags::RDONLY)?;
        Ok(ConfigTemp {
            identity: FileIdentity::from_file(&file)?,
            file,
            name: name.to_os_string(),
        })
    }
    pub(crate) fn create_config_temp(
        &self,
    ) -> std::result::Result<ConfigTemp, Box<ControlCreateFailure>> {
        self.create_control_temp(".librairii-settings-")
    }
    pub(crate) fn create_control_temp(
        &self,
        prefix: &str,
    ) -> std::result::Result<ConfigTemp, Box<ControlCreateFailure>> {
        for _ in 0..64 {
            let name = OsString::from(format!(
                "{prefix}{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match self.create_control_record(&name) {
                Ok(temporary) => return Ok(temporary),
                Err(failure)
                    if failure.allocation.file.is_none()
                        && matches!(&failure.cause,
                    StorageError::Io { source, .. } if source.kind() == io::ErrorKind::AlreadyExists) =>
                {
                    continue;
                }
                Err(failure) => return Err(failure),
            }
        }
        Err(Box::new(ControlCreateFailure {
            cause: StorageError::DestinationCollision,
            allocation: ControlAllocation {
                file: None,
                name: OsString::from(prefix),
            },
        }))
    }
    pub(crate) fn remove_config_temp(&self, temporary: &ConfigTemp) -> Result<()> {
        require_regular(&temporary.file)?;
        self.require_name(&temporary.name, temporary.identity)?;
        native::unlinkat(&self.file, &temporary.name, AtFlags::empty())
            .map_err(|e| StorageError::io("remove owned config temporary", e.into()))?;
        // This source name is private under the profile namespace OS lock.
        // Substitution is refused, and no recursive/path-based cleanup occurs.
        if temporary
            .file
            .metadata()
            .map_err(|e| StorageError::io("check config temporary removal", e))?
            .nlink()
            != 0
        {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(())
    }
    pub(crate) fn replace_owned_config(
        &self,
        temporary: ConfigTemp,
        destination: &OsStr,
        expected: Option<&std::fs::Metadata>,
    ) -> std::result::Result<(ConfigTemp, Option<ConfigTemp>), Box<ConfigReplaceFailure>> {
        self.replace_config_with_checkpoint(temporary, destination, expected, || Ok(()))
    }
    fn replace_config_with_checkpoint(
        &self,
        mut temporary: ConfigTemp,
        destination: &OsStr,
        expected: Option<&std::fs::Metadata>,
        before_rename: impl FnOnce() -> Result<()>,
    ) -> std::result::Result<(ConfigTemp, Option<ConfigTemp>), Box<ConfigReplaceFailure>> {
        let before = (|| {
            EntryName::parse(destination)?;
            require_regular(&temporary.file)?;
            self.require_name(&temporary.name, temporary.identity)?;
            if let Some(expected) = expected {
                self.require_name(
                    destination,
                    FileIdentity {
                        device: expected.dev(),
                        inode: expected.ino(),
                    },
                )?;
            }
            durable_file_flush(&temporary.file)?;
            before_rename()
        })();
        if let Err(cause) = before {
            return Err(Box::new(ConfigReplaceFailure {
                temporary,
                changed: false,
                cause,
            }));
        }
        // With an existing config, EXCHANGE preserves the displaced object in
        // the private temporary name. A concurrent substituted config is never
        // destroyed by rename-overwrite, and is never eligible for cleanup.
        let flags = if expected.is_some() {
            RenameFlags::EXCHANGE
        } else {
            RenameFlags::NOREPLACE
        };
        if let Err(error) =
            native::renameat_with(&self.file, &temporary.name, &self.file, destination, flags)
        {
            return Err(Box::new(ConfigReplaceFailure {
                temporary,
                changed: error != rustix::io::Errno::EXIST,
                cause: StorageError::io("replace owned config atomically", error.into()),
            }));
        }
        let displaced_name = temporary.name.clone();
        temporary.name = destination.to_os_string();
        let after = (|| {
            self.require_name(destination, temporary.identity)?;
            if let Some(expected) = expected {
                let identity = FileIdentity {
                    device: expected.dev(),
                    inode: expected.ino(),
                };
                self.require_name(&displaced_name, identity)?;
                let file = self.open_regular(&displaced_name, OFlags::RDONLY)?;
                Ok(Some(ConfigTemp {
                    file,
                    name: displaced_name,
                    identity,
                }))
            } else {
                Ok(None)
            }
        })();
        match after {
            Ok(old) => Ok((temporary, old)),
            Err(cause) => Err(Box::new(ConfigReplaceFailure {
                temporary,
                changed: true,
                cause,
            })),
        }
    }
}

/// File-data durability is independent of atomic name publication and parent
/// directory durability. macOS additionally requests the actual device flush;
/// unsupported filesystems return the native error, without an fsync fallback.
pub fn durable_file_flush(file: &File) -> Result<()> {
    file.sync_all()
        .map_err(|e| StorageError::io("flush owned file", e))?;
    #[cfg(target_os = "macos")]
    native::fcntl_fullfsync(file)
        .map_err(|e| StorageError::io("flush owned file to device", e.into()))?;
    Ok(())
}

fn require_regular(file: &File) -> Result<()> {
    let stat = file
        .metadata()
        .map_err(|e| StorageError::io("inspect regular file witness", e))?;
    if !stat.is_file() || stat.nlink() != 1 {
        return Err(StorageError::PathEscape);
    }
    Ok(())
}

fn unique_name(prefix: &str) -> OsString {
    // Randomness is not authority. mkdirat/RENAME_NOREPLACE establish exclusive
    // creation; collisions are bounded refusals and never adopt existing names.
    format!(
        ".librairii-{prefix}-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileArea {
    Archives,
    Catalog,
}

/// Owned-handle observations only: equality cannot prove an adversarial
/// source snapshot or exclusion. No observation creates a file capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadSourceObservation {
    parent: FileIdentity,
    file: FileIdentity,
    bytes: u64,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl ReadSourceObservation {
    pub const fn parent_identity(self) -> FileIdentity {
        self.parent
    }
    pub const fn identity(self) -> FileIdentity {
        self.file
    }
    /// Observed current size, NOT a trusted immutable size or product limit.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
    pub const fn links(self) -> u64 {
        self.links
    }
}

/// A read-only regular source issued by an actual retained managed-parent
/// owner. It is NOT general external-picker permission, source write/cleanup,
/// an immutable snapshot, a stage or a custody/publication capability.
///
/// The actual Files/Database/library leases cannot close before this owner:
/// ```compile_fail
/// use librairii_storage::{Result, database::Database,
///     platform::unix::{LibraryFiles, FileArea, EntryName}};
/// fn close_before_source(db: &mut Database<'_>) -> Result<()> {
///     let files = LibraryFiles::open_native(db)?;
///     let source = files.open_read_source(FileArea::Archives, EntryName::parse("owned")?)?;
///     db.close()?;
///     drop(source);
///     Ok(())
/// }
/// ```
/// A source also cannot discard its Files owner and keep only an extracted
/// root/path/ID as a substitute for the real database borrow:
/// ```compile_fail
/// use librairii_storage::{Result, database::Database,
///     platform::unix::{LibraryFiles, FileArea, EntryName}};
/// fn source_after_files(db: &mut Database<'_>) -> Result<()> {
///     let files = LibraryFiles::open_native(db)?;
///     let source = files.open_read_source(FileArea::Archives, EntryName::parse("owned")?)?;
///     drop(files);
///     db.close()?;
///     drop(source);
///     Ok(())
/// }
/// ```
/// No public write implementation or writable File extraction is supplied:
/// ```compile_fail
/// use std::io::Write;
/// use librairii_storage::platform::unix::OwnedReadSource;
/// fn cannot_write(source: &mut OwnedReadSource<'_, '_>) {
///     let _ = source.write_all(b"not a writable source");
/// }
/// ```
#[derive(Debug)]
pub struct OwnedReadSource<'files, 'library> {
    file: File,
    parent: Directory,
    files: &'files LibraryFiles<'library>,
    area: FileArea,
    name: EntryName,
    initial: ReadSourceObservation,
}
// Keep the real Files/DB/library borrow through actual FD/parent close. No
// filesystem effect, cancellation, compensation or fence reset occurs here.
impl Drop for OwnedReadSource<'_, '_> {
    fn drop(&mut self) {}
}
impl OwnedReadSource<'_, '_> {
    pub fn name(&self) -> &EntryName {
        &self.name
    }
    pub const fn initial(&self) -> ReadSourceObservation {
        self.initial
    }
    /// Re-observe the SAME retained source and parent/name relationship. This
    /// readonly observation remains possible for drain after intake stops or
    /// a generation fences; it neither clears the fence nor grants new effects.
    pub fn observe(&self) -> Result<ReadSourceObservation> {
        match self.files.authority {
            FilesAuthority::Isolated(library) => library.revalidate_database_ownership()?,
            FilesAuthority::Native { library, .. } => library.revalidate_database_ownership()?,
        }
        let area_name = match self.area {
            FileArea::Archives => OsStr::new("archives"),
            FileArea::Catalog => OsStr::new("catalog"),
        };
        self.files
            .root
            .require_name(area_name, self.parent.identity)?;
        if FileIdentity::from_file(&self.parent.file)? != self.parent.identity {
            return Err(StorageError::SnapshotChanged);
        }
        let stat = self
            .file
            .metadata()
            .map_err(|e| StorageError::io("observe retained read source", e))?;
        if !stat.is_file() || stat.nlink() != 1 {
            return Err(StorageError::PathEscape);
        }
        let file = FileIdentity {
            device: stat.dev(),
            inode: stat.ino(),
        };
        if file != self.initial.file {
            return Err(StorageError::SnapshotChanged);
        }
        self.parent.require_name(self.name.as_os_str(), file)?;
        Ok(ReadSourceObservation {
            parent: self.parent.identity,
            file,
            bytes: stat.len(),
            links: stat.nlink(),
            modified: (stat.mtime(), stat.mtime_nsec()),
            changed: (stat.ctime(), stat.ctime_nsec()),
        })
    }
    /// Compare cooperative before/after metadata observations. An in-place
    /// writer may evade such checks; no adversarial immutability is claimed.
    pub fn revalidate(&self) -> Result<ReadSourceObservation> {
        let current = self.observe()?;
        if current != self.initial {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(current)
    }
}
impl Read for OwnedReadSource<'_, '_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.revalidate().map_err(io::Error::other)?;
        self.file.read(bytes)
    }
}
impl Seek for OwnedReadSource<'_, '_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.revalidate().map_err(io::Error::other)?;
        self.file.seek(position)
    }
}

#[derive(Clone, Copy, Debug)]
enum FilesAuthority<'library> {
    Isolated(&'library IsolatedLibrary),
    Native {
        library: &'library NativeLibrary<'library>,
        // Retain the SAME live DB borrow until files/guards/receipts close.
        database: &'library Database<'library>,
    },
}
impl<'library> FilesAuthority<'library> {
    fn active(self) -> &'library AtomicBool {
        match self {
            Self::Isolated(library) => &library.custody_active,
            Self::Native { library, .. } => &library.custody_active,
        }
    }
    fn fenced(self) -> bool {
        match self {
            Self::Isolated(library) => library.custody_fenced.load(Ordering::Acquire),
            Self::Native { library, .. } => library.effect_fence().load(Ordering::Acquire),
        }
    }
    fn fence(self) {
        match self {
            Self::Isolated(library) => library.custody_fenced.store(true, Ordering::Release),
            Self::Native { library, .. } => library.fence(),
        }
    }
    fn same_generation(self, other: Self) -> bool {
        match (self, other) {
            (Self::Isolated(a), Self::Isolated(b)) => std::ptr::eq(a, b),
            (Self::Native { library: a, .. }, Self::Native { library: b, .. }) => {
                std::ptr::eq(a, b)
            }
            _ => false,
        }
    }
    fn require_intake(self) -> Result<()> {
        match self {
            Self::Isolated(_) => Ok(()),
            Self::Native { database, .. } => database.require_native_file_intake(),
        }
    }
    fn revalidate(self) -> Result<()> {
        match self {
            Self::Isolated(library) => library.revalidate_ownership(),
            Self::Native { library, database } => {
                database.native_file_owner()?;
                library.revalidate(&WorkCancellation::default())
            }
        }
    }
}

/// Library generation is represented by an actual borrow of the authority, not
/// its UUID. Multiple filesystem handles share the SAME all-claimant guard.
///
/// The real lease cannot close while a stage still owns open resources:
/// ```compile_fail
/// use librairii_storage::{IsolatedLibrary, Result, platform::unix::LibraryFiles};
/// fn release_early(library: IsolatedLibrary) -> Result<()> {
///     let files = LibraryFiles::open(&library)?;
///     let guard = files.try_custody()?;
///     let staged = files.create_stage(&guard);
///     drop(guard);
///     drop(files); // the retained stage still borrows the directory owners
///     drop(library);
///     drop(staged);
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct LibraryFiles<'library> {
    authority: FilesAuthority<'library>,
    root: Directory,
    staging: Directory,
    archives: Directory,
    catalog: Directory,
    catalog_route: Option<ArtworkRoute>,
    archive_route: Option<ArchiveRoute>,
    trash: Directory,
}
#[derive(Debug)]
struct ArtworkRoute {
    root_name: &'static str,
    id: String,
    category: Option<Directory>,
    artwork: Option<Directory>,
}
#[derive(Debug)]
struct ArchiveRoute {
    hash: librairii_core::Checksum,
    directory: Option<Directory>,
}
impl LibraryFiles<'_> {
    /// A closed, hash-typed catalog route, under the same retained custody.
    /// Parent directories are durable before the existing publisher is entered.
    pub(crate) fn official_artwork(
        &mut self,
        guard: &Custody<'_>,
        id: librairii_core::Checksum,
    ) -> Result<()> {
        self.artwork_route(guard, "official", id.to_string())
    }
    pub(crate) fn embedded_artwork(
        &mut self,
        guard: &Custody<'_>,
        uuid: librairii_core::StoryUuid,
    ) -> Result<()> {
        self.artwork_route(guard, "embedded", uuid.to_string())
    }
    fn artwork_route(
        &mut self,
        guard: &Custody<'_>,
        root_name: &'static str,
        id: String,
    ) -> Result<()> {
        self.check(guard)?;
        let ensure = |parent: &Directory, name: &OsStr| -> Result<Directory> {
            match parent.child(name) {
                Ok(dir) => Ok(dir),
                Err(StorageError::Io { source, .. })
                    if source.kind() == io::ErrorKind::NotFound =>
                {
                    parent.create_child(name)
                }
                Err(e) => Err(e),
            }
        };
        self.catalog_route = Some(ArtworkRoute {
            root_name,
            id,
            category: None,
            artwork: None,
        });
        let result = (|| {
            let route = self
                .catalog_route
                .as_mut()
                .ok_or(StorageError::CompatibilityInvariant)?;
            route.category = Some(ensure(&self.catalog, OsStr::new(root_name))?);
            let category = route
                .category
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            route.artwork = Some(ensure(category, OsStr::new(&route.id))?);
            self.catalog.flush()?;
            category.flush()?;
            route
                .artwork
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?
                .flush()?;
            self.check(guard)
        })();
        if result.is_err() {
            self.fence();
        }
        result?;
        Ok(())
    }
}
// Keep the library borrow live until all retained directory descriptors close.
// This destructor performs no filesystem mutation; fields close normally.
impl Drop for LibraryFiles<'_> {
    fn drop(&mut self) {}
}

/// Move-only exclusion. It is Send and can participate in a later explicit
/// resource handoff; there is no timeout/expiry or fabricated transfer by ID.
#[derive(Debug)]
pub struct Custody<'library> {
    authority: FilesAuthority<'library>,
}
impl Drop for Custody<'_> {
    fn drop(&mut self) {
        self.authority.active().store(false, Ordering::Release);
        // custody_fenced is deliberately NOT cleared here. No current route
        // activates an existing/recovery library without independent validation.
    }
}

impl<'library> LibraryFiles<'library> {
    pub fn open(authority: &'library IsolatedLibrary) -> Result<Self> {
        authority.revalidate_ownership()?;
        let root = Directory::open(authority.layout().root())?;
        let expected = std::fs::symlink_metadata(authority.layout().root())
            .map_err(|e| StorageError::io("validate anchored root identity", e))?;
        if !expected.is_dir()
            || expected.file_type().is_symlink()
            || root.identity
                != (FileIdentity {
                    device: expected.dev(),
                    inode: expected.ino(),
                })
        {
            return Err(StorageError::LibraryLeaseChanged);
        }
        Self::from_root(FilesAuthority::Isolated(authority), root)
    }

    /// Native file effects borrow the actual open-native Database, its sealed
    /// independent library, profile binding and OS leases. No path/ID/DTO can
    /// supply this authority, and a development Database is refused.
    ///
    /// Mutable close/join cannot precede the real file-owner drain:
    /// ```compile_fail
    /// use librairii_storage::{Result, database::Database, platform::unix::LibraryFiles};
    /// fn close_early(db: &mut Database<'_>) -> Result<()> {
    ///     let files = LibraryFiles::open_native(db)?;
    ///     let guard = files.try_custody()?;
    ///     let stage = files.create_stage(&guard);
    ///     db.close()?;
    ///     drop(stage);
    ///     Ok(())
    /// }
    /// ```
    /// A guard can outlive a Files instance, but cannot lose the DB borrow:
    /// ```compile_fail
    /// use librairii_storage::{Result, database::Database, platform::unix::LibraryFiles};
    /// fn guard_after_files(db: &mut Database<'_>) -> Result<()> {
    ///     let files = LibraryFiles::open_native(db)?;
    ///     let guard = files.try_custody()?;
    ///     drop(files);
    ///     db.close()?;
    ///     drop(guard);
    ///     Ok(())
    /// }
    /// ```
    /// Success, reuse and uncertainty receipts retain the Files owner too:
    /// ```compile_fail
    /// use librairii_storage::{Result, database::{Database, WorkCancellation},
    ///     platform::unix::{LibraryFiles, EntryName, FileArea}};
    /// fn receipt_after_files(db: &mut Database<'_>) -> Result<()> {
    ///     let files = LibraryFiles::open_native(db)?;
    ///     let guard = files.try_custody()?;
    ///     let mut stage = {
    ///         let creation = files.create_stage(&guard);
    ///         assert!(creation.is_ok(), "successful-stage fixture required");
    ///         match creation { Ok(stage) => stage, Err(_retained) => return Ok(()) }
    ///     };
    ///     let cancel = WorkCancellation::default();
    ///     let digest = stage.measure(0, &cancel)?;
    ///     let receipt = stage.publish_no_clobber(&guard, FileArea::Archives,
    ///         EntryName::parse("owned.bin")?, digest, &cancel);
    ///     drop(files);
    ///     db.close()?;
    ///     drop(receipt);
    ///     Ok(())
    /// }
    /// ```
    pub fn open_native(database: &'library Database<'_>) -> Result<Self> {
        database.require_native_file_intake()?;
        let library = database.native_file_owner()?;
        let root = library.retain_active_files_root()?;
        Self::from_root(FilesAuthority::Native { library, database }, root)
    }

    fn from_root(authority: FilesAuthority<'library>, root: Directory) -> Result<Self> {
        let files = Self {
            staging: root.child(OsStr::new("staging"))?,
            archives: root.child(OsStr::new("archives"))?,
            catalog: root.child(OsStr::new("catalog"))?,
            catalog_route: None,
            archive_route: None,
            trash: root.child(OsStr::new("trash"))?,
            root,
            authority,
        };
        files.revalidate()?;
        Ok(files)
    }
    pub fn try_custody(&self) -> Result<Custody<'library>> {
        if self.authority.fenced() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        self.authority.require_intake()?;
        self.authority
            .active()
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| StorageError::CustodyBusy)?;
        let guard = Custody {
            authority: self.authority,
        };
        self.check(&guard)?;
        Ok(guard)
    }
    fn check(&self, guard: &Custody<'_>) -> Result<()> {
        if !self.authority.same_generation(guard.authority) {
            return Err(StorageError::IdentityMismatch);
        }
        if self.authority.fenced() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        self.revalidate()
    }
    fn revalidate(&self) -> Result<()> {
        self.authority.revalidate()?;
        for (name, directory) in [
            ("staging", &self.staging),
            ("archives", &self.archives),
            ("catalog", &self.catalog),
            ("trash", &self.trash),
        ] {
            self.root
                .require_name(OsStr::new(name), directory.identity)?;
            if FileIdentity::from_file(&directory.file)? != directory.identity {
                return Err(StorageError::SnapshotChanged);
            }
        }
        if let Some(route) = &self.catalog_route {
            let category = route
                .category
                .as_ref()
                .ok_or(StorageError::CustodyRecoveryRequired)?;
            let artwork = route
                .artwork
                .as_ref()
                .ok_or(StorageError::CustodyRecoveryRequired)?;
            self.catalog
                .require_name(OsStr::new(route.root_name), category.identity)?;
            category.require_name(OsStr::new(&route.id), artwork.identity)?;
        }
        if let Some(route) = &self.archive_route {
            let directory = route
                .directory
                .as_ref()
                .ok_or(StorageError::CustodyRecoveryRequired)?;
            self.archives
                .require_name(OsStr::new(&route.hash.to_string()), directory.identity)?;
        }
        Ok(())
    }
    pub(crate) fn fence_metadata(&self) {
        self.fence();
    }
    fn fence(&self) {
        self.authority.fence();
    }
    fn area(&self, area: FileArea) -> &Directory {
        match area {
            FileArea::Archives => self
                .archive_route
                .as_ref()
                .and_then(|route| route.directory.as_ref())
                .unwrap_or(&self.archives),
            FileArea::Catalog => self
                .catalog_route
                .as_ref()
                .and_then(|route| route.artwork.as_ref())
                .unwrap_or(&self.catalog),
        }
    }
    /// Issue only from an actual storage-owned managed-area descriptor and one
    /// validated lossless component. No arbitrary Path/File/metadata/TestRoots
    /// constructor or general external-picker authority is provided.
    pub fn open_read_source(
        &self,
        area: FileArea,
        name: EntryName,
    ) -> Result<OwnedReadSource<'_, 'library>> {
        if self.authority.fenced() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        self.authority.require_intake()?;
        self.revalidate()?;
        let parent = self.area(area).duplicate()?;
        // The existing safe descriptor-relative route fixes O_RDONLY plus
        // NOFOLLOW|NONBLOCK|CLOEXEC; regular/nlink1 and name-to-FD are checked.
        let file = parent.open_regular(name.as_os_str(), OFlags::RDONLY)?;
        let stat = file
            .metadata()
            .map_err(|e| StorageError::io("observe initial read source", e))?;
        let initial = ReadSourceObservation {
            parent: parent.identity,
            file: FileIdentity::from_file(&file)?,
            bytes: stat.len(),
            links: stat.nlink(),
            modified: (stat.mtime(), stat.mtime_nsec()),
            changed: (stat.ctime(), stat.ctime_nsec()),
        };
        let source = OwnedReadSource {
            file,
            parent,
            files: self,
            area,
            name,
            initial,
        };
        source.revalidate()?;
        self.authority.require_intake()?;
        if self.authority.fenced() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        Ok(source)
    }
    /// The boxed failure retains the SAME Files and Database borrow even after
    /// the Files variable's last ordinary use. This fails for the actual owner
    /// borrow, without any `?`/error conversion on stage creation:
    /// ```compile_fail
    /// use librairii_storage::{Result, database::Database, platform::unix::LibraryFiles};
    /// fn failed_creation(db: &mut Database<'_>) -> Result<()> {
    ///     let files = LibraryFiles::open_native(db)?;
    ///     let guard = files.try_custody()?;
    ///     let retained = match files.create_stage(&guard) {
    ///         Err(retained) => retained,
    ///         Ok(_stage) => return Ok(()),
    ///     };
    ///     drop(files);
    ///     db.close()?;
    ///     drop(retained);
    ///     Ok(())
    /// }
    /// ```
    /// Compile-pass control: both returned variants can retain the owner and
    /// allow exclusive close ONLY after their actual borrows end. This example
    /// is type checking, not settlement/cleanup qualification of a failure:
    /// ```
    /// use librairii_storage::{Result, database::Database, platform::unix::LibraryFiles};
    /// fn owner_order(db: &mut Database<'_>) -> Result<()> {
    ///     let files = LibraryFiles::open_native(db)?;
    ///     let guard = files.try_custody()?;
    ///     let creation = files.create_stage(&guard);
    ///     drop(creation);
    ///     drop(guard);
    ///     drop(files);
    ///     db.close()?;
    ///     Ok(())
    /// }
    /// ```
    /// Success fixture control for the publication-receipt lifetime example:
    /// ```
    /// use librairii_storage::{Result, database::{Database, WorkCancellation},
    ///     platform::unix::{LibraryFiles, EntryName, FileArea}};
    /// fn receipt_owner_order(db: &mut Database<'_>) -> Result<()> {
    ///     let files = LibraryFiles::open_native(db)?;
    ///     let guard = files.try_custody()?;
    ///     let mut stage = {
    ///         let creation = files.create_stage(&guard);
    ///         assert!(creation.is_ok(), "successful-stage fixture required");
    ///         match creation { Ok(stage) => stage, Err(_retained) => return Ok(()) }
    ///     };
    ///     let cancel = WorkCancellation::default();
    ///     let digest = stage.measure(0, &cancel)?;
    ///     let receipt = stage.publish_no_clobber(&guard, FileArea::Archives,
    ///         EntryName::parse("owned.bin")?, digest, &cancel);
    ///     drop(receipt);
    ///     drop(guard);
    ///     drop(files);
    ///     db.close()?;
    ///     Ok(())
    /// }
    /// ```
    /// Create an owned stage or return the actual retained creation failure.
    /// A partial attempt has no cleanup/publication capability. Keep it and the
    /// original custody owner until a separately qualified resolution exists.
    /// No conversion erases its Files/Database/lease borrow into StorageError.
    pub fn create_stage<'files>(
        &'files self,
        guard: &Custody<'_>,
    ) -> std::result::Result<OwnedStage<'files, 'library>, Box<StageCreateFailure<'files, 'library>>>
    {
        self.create_stage_with(guard, || unique_name("stage"), |_, _| Ok(()))
    }

    fn create_stage_with<'files>(
        &'files self,
        guard: &Custody<'_>,
        next_name: impl FnMut() -> OsString,
        checkpoint: impl FnMut(StageCreatePoint, &StageAllocation<'files, 'library>) -> Result<()>,
    ) -> std::result::Result<OwnedStage<'files, 'library>, Box<StageCreateFailure<'files, 'library>>>
    {
        Self::create_stage_from(FileLoan::Borrowed(self), guard, next_name, checkpoint)
    }
    pub(crate) fn create_owned_stage(
        files: std::sync::Arc<Self>,
        guard: &Custody<'_>,
    ) -> std::result::Result<
        OwnedStage<'library, 'library>,
        Box<StageCreateFailure<'library, 'library>>,
    > {
        Self::create_stage_from(
            FileLoan::Owned(files),
            guard,
            || unique_name("stage"),
            |_, _| Ok(()),
        )
    }
    #[cfg(test)]
    pub(crate) fn create_owned_stage_fault(
        files: std::sync::Arc<Self>,
        guard: &Custody<'_>,
    ) -> std::result::Result<
        OwnedStage<'library, 'library>,
        Box<StageCreateFailure<'library, 'library>>,
    > {
        Self::create_stage_from(
            FileLoan::Owned(files),
            guard,
            || unique_name("stage"),
            |point, _| {
                if point == StageCreatePoint::PayloadAcquired {
                    Err(StorageError::IntegrityFailed)
                } else {
                    Ok(())
                }
            },
        )
    }
    fn create_stage_from<'files>(
        files: FileLoan<'files, 'library>,
        guard: &Custody<'_>,
        mut next_name: impl FnMut() -> OsString,
        mut checkpoint: impl FnMut(StageCreatePoint, &StageAllocation<'files, 'library>) -> Result<()>,
    ) -> std::result::Result<OwnedStage<'files, 'library>, Box<StageCreateFailure<'files, 'library>>>
    {
        files
            .check(guard)
            .and_then(|()| files.authority.require_intake())
            .map_err(StageCreateFailure::no_allocation)?;
        for _ in 0..64 {
            // All original parent/name witnesses exist BEFORE the first effect.
            let mut allocation = StageAllocation {
                files: files.clone(),
                directory_name: next_name(),
                payload_name: OsString::from(PAYLOAD),
                directory: None,
                payload: None,
                directory_identity: None,
                payload_identity: None,
                mkdir_attempted: false,
                directory_created: false,
                payload_open_attempted: false,
                payload_created: false,
                known_no_create: false,
            };
            if let Err(cause) = allocation.acquire(guard, &mut checkpoint) {
                if allocation.known_no_create {
                    // EEXIST owns NO colliding/replacement object. No retry
                    // after any successful or ambiguous allocation is allowed.
                    continue;
                }
                if !allocation.mkdir_attempted {
                    return Err(StageCreateFailure::no_allocation(cause));
                }
                files.fence();
                return Err(Box::new(StageCreateFailure {
                    cause,
                    allocation: Some(allocation),
                }));
            }
            // All four actual acquisitions/observations were checked above.
            // Retain every available field even on an internal invariant error.
            match (
                allocation.directory.take(),
                allocation.directory_identity,
                allocation.payload.take(),
                allocation.payload_identity,
            ) {
                (Some(directory), Some(directory_identity), Some(file), Some(identity)) => {
                    return Ok(OwnedStage {
                        files: files.clone(),
                        directory: Directory {
                            file: directory,
                            identity: directory_identity,
                        },
                        directory_name: allocation.directory_name.clone(),
                        file,
                        identity,
                        published: false,
                        registration: None,
                    });
                }
                (directory, _, payload, _) => {
                    allocation.directory = directory;
                    allocation.payload = payload;
                    files.fence();
                    return Err(Box::new(StageCreateFailure {
                        cause: StorageError::CompatibilityInvariant,
                        allocation: Some(allocation),
                    }));
                }
            }
        }
        Err(StageCreateFailure::no_allocation(
            StorageError::DestinationCollision,
        ))
    }

    #[cfg(test)]
    fn create_stage_after_open(
        &self,
        guard: &Custody<'_>,
        mut after_open: impl FnMut(&OsStr, &File) -> Result<()>,
    ) -> std::result::Result<OwnedStage<'_, 'library>, Box<StageCreateFailure<'_, 'library>>> {
        self.create_stage_with(
            guard,
            || unique_name("stage"),
            |point, allocation| {
                if point == StageCreatePoint::PayloadAcquired {
                    after_open(&allocation.directory_name, allocation.payload_file()?)?;
                }
                Ok(())
            },
        )
    }
}

/// A positively no-allocation result concerns THIS call only. It neither
/// clears earlier uncertainty/fences nor owns any colliding object's bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageCreateDisposition {
    NoAllocation,
    RetainedUnresolved,
}

/// The original structured cause and actual partial owner are independent.
/// There is deliberately no Error/From conversion to a bare storage error.
/// Dropping this value closes handles, preserves bytes and NEVER clears fences.
#[derive(Debug)]
pub struct StageCreateFailure<'files, 'library> {
    cause: StorageError,
    allocation: Option<StageAllocation<'files, 'library>>,
}
impl<'files, 'library> StageCreateFailure<'files, 'library> {
    fn no_allocation(cause: StorageError) -> Box<Self> {
        Box::new(Self {
            cause,
            allocation: None,
        })
    }
    pub fn cause(&self) -> &StorageError {
        &self.cause
    }
    pub fn disposition(&self) -> StageCreateDisposition {
        if self.allocation.is_some() {
            StageCreateDisposition::RetainedUnresolved
        } else {
            StageCreateDisposition::NoAllocation
        }
    }
    pub fn retained_allocation(&self) -> Option<&StageAllocation<'files, 'library>> {
        self.allocation.as_ref()
    }
}

#[derive(Clone, Debug)]
enum FileLoan<'files, 'library> {
    Borrowed(&'files LibraryFiles<'library>),
    Owned(std::sync::Arc<LibraryFiles<'library>>),
}
impl<'library> std::ops::Deref for FileLoan<'_, 'library> {
    type Target = LibraryFiles<'library>;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(files) => files,
            Self::Owned(files) => files,
        }
    }
}
/// Opaque, unresolved attempted creation. Actual optional raw handles are held
/// before ANY fallible metadata/type/name validation. Missing observations stay
/// None. Names/IDs are observations, never authority over substituted objects.
/// No public write/cleanup/raw-handle extraction or partial retirement exists.
#[derive(Debug)]
pub struct StageAllocation<'files, 'library> {
    files: FileLoan<'files, 'library>,
    directory_name: OsString,
    payload_name: OsString,
    directory: Option<File>,
    payload: Option<File>,
    directory_identity: Option<FileIdentity>,
    payload_identity: Option<FileIdentity>,
    mkdir_attempted: bool,
    directory_created: bool,
    payload_open_attempted: bool,
    payload_created: bool,
    known_no_create: bool,
}
// Keep the actual Files/DB/Native/lease borrow until retained descriptors close.
// This performs NO filesystem effects or resolution.
impl Drop for StageAllocation<'_, '_> {
    fn drop(&mut self) {}
}
impl StageAllocation<'_, '_> {
    #[cfg(test)]
    pub(crate) fn retained_payload_metadata(&self) -> Result<Option<std::fs::Metadata>> {
        self.payload
            .as_ref()
            .map(|file| {
                file.metadata()
                    .map_err(|e| StorageError::io("inspect actual retained payload", e))
            })
            .transpose()
    }
    pub fn parent_identity(&self) -> FileIdentity {
        self.files.staging.identity
    }
    pub fn directory_name(&self) -> &OsStr {
        &self.directory_name
    }
    pub fn payload_name(&self) -> &OsStr {
        &self.payload_name
    }
    pub fn mkdir_attempted(&self) -> bool {
        self.mkdir_attempted
    }
    /// Positive mkdir success. False after an attempt is UNKNOWN, not proof
    /// of no allocation; the owning failure's disposition remains unresolved.
    pub fn directory_created(&self) -> bool {
        self.directory_created
    }
    pub fn payload_open_attempted(&self) -> bool {
        self.payload_open_attempted
    }
    /// Positive returned creation handle. False after open was attempted does
    /// not certify no effects and never grants ownership of a collision.
    pub fn payload_created(&self) -> bool {
        self.payload_created
    }
    pub fn directory_acquired(&self) -> bool {
        self.directory.is_some()
    }
    pub fn payload_acquired(&self) -> bool {
        self.payload.is_some()
    }
    pub fn directory_identity(&self) -> Option<FileIdentity> {
        self.directory_identity
    }
    pub fn payload_identity(&self) -> Option<FileIdentity> {
        self.payload_identity
    }

    fn directory_file(&self) -> Result<&File> {
        self.directory
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)
    }
    fn payload_file(&self) -> Result<&File> {
        self.payload
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)
    }
    fn acquire(
        &mut self,
        guard: &Custody<'_>,
        checkpoint: &mut impl FnMut(StageCreatePoint, &Self) -> Result<()>,
    ) -> Result<()> {
        EntryName::parse(self.directory_name.clone())?;
        EntryName::parse(self.payload_name.clone())?;
        // A no-create collision never grants a later attempt permission to
        // ignore a concurrently established generation/intake fence.
        self.files.check(guard)?;
        self.files.authority.require_intake()?;
        checkpoint(StageCreatePoint::BeforeMkdir, self)?;
        self.mkdir_attempted = true;
        match native::mkdirat(&self.files.staging.file, &self.directory_name, Mode::RWXU) {
            Ok(()) => self.directory_created = true,
            Err(error) => {
                self.known_no_create = error == rustix::io::Errno::EXIST;
                return Err(StorageError::io(
                    "create exclusive stage directory",
                    error.into(),
                ));
            }
        }
        checkpoint(StageCreatePoint::AfterMkdir, self)?;
        // Do not use bare-error Directory::child/from_file: capture FIRST.
        self.directory = Some(
            native::openat(
                &self.files.staging.file,
                &self.directory_name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| StorageError::io("anchor created stage directory", e.into()))?
            .into(),
        );
        checkpoint(StageCreatePoint::DirectoryAcquired, self)?;
        let metadata = self
            .directory_file()?
            .metadata()
            .map_err(|e| StorageError::io("inspect created stage directory", e))?;
        checkpoint(StageCreatePoint::DirectoryType, self)?;
        if !metadata.is_dir() {
            return Err(StorageError::PathEscape);
        }
        checkpoint(StageCreatePoint::DirectoryIdentity, self)?;
        self.directory_identity = Some(FileIdentity::from_file(self.directory_file()?)?);
        checkpoint(StageCreatePoint::DirectoryName, self)?;
        self.files.staging.require_name(
            &self.directory_name,
            self.directory_identity
                .ok_or(StorageError::CompatibilityInvariant)?,
        )?;
        checkpoint(StageCreatePoint::BeforePayloadOpen, self)?;
        self.payload_open_attempted = true;
        // Capture the returned raw FD before require_regular/fstat/name checks.
        self.payload = Some(
            native::openat(
                self.directory_file()?,
                &self.payload_name,
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|e| StorageError::io("create exclusive stage payload", e.into()))?
            .into(),
        );
        self.payload_created = true;
        checkpoint(StageCreatePoint::PayloadAcquired, self)?;
        let metadata = self
            .payload_file()?
            .metadata()
            .map_err(|e| StorageError::io("inspect created stage payload", e))?;
        checkpoint(StageCreatePoint::PayloadType, self)?;
        if !metadata.is_file() {
            return Err(StorageError::PathEscape);
        }
        checkpoint(StageCreatePoint::PayloadLinks, self)?;
        if metadata.nlink() != 1 {
            return Err(StorageError::PathEscape);
        }
        checkpoint(StageCreatePoint::PayloadIdentity, self)?;
        self.payload_identity = Some(FileIdentity::from_file(self.payload_file()?)?);
        checkpoint(StageCreatePoint::PayloadName, self)?;
        let stat = native::statat(
            self.directory_file()?,
            &self.payload_name,
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|e| StorageError::io("revalidate created stage payload", e.into()))?;
        if Some(FileIdentity::from_stat(&stat)) != self.payload_identity {
            return Err(StorageError::SnapshotChanged);
        }
        checkpoint(StageCreatePoint::FinalCheck, self)?;
        // Revalidate both the original parent/name and the payload after all
        // callbacks/checks. Never follow a replacement directory for effects.
        self.files.staging.require_name(
            &self.directory_name,
            self.directory_identity
                .ok_or(StorageError::CompatibilityInvariant)?,
        )?;
        let payload_identity = self
            .payload_identity
            .ok_or(StorageError::CompatibilityInvariant)?;
        require_regular(self.payload_file()?)?;
        let payload = native::statat(
            self.directory_file()?,
            &self.payload_name,
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|e| StorageError::io("final created stage payload check", e.into()))?;
        if FileIdentity::from_stat(&payload) != payload_identity {
            return Err(StorageError::SnapshotChanged);
        }
        self.files.check(guard)?;
        self.files.authority.require_intake()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StageCreatePoint {
    BeforeMkdir,
    AfterMkdir,
    DirectoryAcquired,
    DirectoryType,
    DirectoryIdentity,
    DirectoryName,
    BeforePayloadOpen,
    PayloadAcquired,
    PayloadType,
    PayloadLinks,
    PayloadIdentity,
    PayloadName,
    FinalCheck,
}

/// This handle owns an exclusively created regular file and private directory.
/// Drop closes handles and PRESERVES bytes. No pathname/hash tuple can create it.
#[derive(Debug)]
pub struct OwnedStage<'files, 'library> {
    files: FileLoan<'files, 'library>,
    directory: Directory,
    directory_name: OsString,
    file: File,
    identity: FileIdentity,
    published: bool,
    registration: Option<Box<crate::native_effects::Temporary>>,
}
// Drop-checking must retain the generation/OS lease until its real file and
// directory handles close, even when a caller stops using the stage early.
impl Drop for OwnedStage<'_, '_> {
    fn drop(&mut self) {}
}
impl Write for OwnedStage<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.published {
            return Err(io::Error::other(
                "published or uncertain stage is read-only",
            ));
        }
        self.revalidate_source().map_err(io::Error::other)?;
        self.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Read/seek access is inspection, never ownership or write/cleanup authority.
#[derive(Debug)]
pub struct StageReader<'stage> {
    file: &'stage mut File,
}
impl Read for StageReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}
impl Seek for StageReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

#[derive(Debug)]
pub struct ExistingFile<'files, 'library> {
    files: FileLoan<'files, 'library>,
    area: FileArea,
    name: EntryName,
    file: File,
    identity: FileIdentity,
    measurement: StreamDigest,
}
impl Drop for ExistingFile<'_, '_> {
    fn drop(&mut self) {}
}
impl ExistingFile<'_, '_> {
    pub fn measurement(&self) -> StreamDigest {
        self.measurement
    }
    pub fn name(&self) -> &EntryName {
        &self.name
    }
    pub fn identity(&self) -> FileIdentity {
        self.identity
    }
    pub fn revalidate(
        &mut self,
        guard: &Custody<'_>,
        cancellation: &WorkCancellation,
    ) -> Result<()> {
        self.files.check(guard)?;
        require_regular(&self.file)?;
        self.file
            .rewind()
            .map_err(|e| StorageError::io("rewind existing receipt", e))?;
        if hash_stream(&mut self.file, self.measurement.bytes(), cancellation)? != self.measurement
        {
            return Err(StorageError::SnapshotChanged);
        }
        self.files
            .area(self.area)
            .require_name(self.name.as_os_str(), self.identity)
    }
}

#[derive(Debug)]
pub struct PublishedFile<'files, 'library> {
    // Retain the original file and source-directory witnesses after rename.
    stage: OwnedStage<'files, 'library>,
    area: FileArea,
    name: EntryName,
    measurement: StreamDigest,
}
impl PublishedFile<'_, '_> {
    pub fn measurement(&self) -> StreamDigest {
        self.measurement
    }
    pub fn name(&self) -> &EntryName {
        &self.name
    }
    pub fn identity(&self) -> FileIdentity {
        self.stage.identity
    }
    /// Rehash/revalidate the actual retained file. This proves neither current
    /// DB references nor import commit status and NEVER permits compensation.
    pub fn revalidate(
        &mut self,
        guard: &Custody<'_>,
        cancellation: &WorkCancellation,
    ) -> Result<()> {
        self.stage.files.check(guard)?;
        require_regular(&self.stage.file)?;
        if self.stage.measure(self.measurement.bytes(), cancellation)? != self.measurement {
            return Err(StorageError::SnapshotChanged);
        }
        self.stage
            .files
            .area(self.area)
            .require_name(self.name.as_os_str(), self.stage.identity)
    }
}

#[derive(Debug)]
pub enum Publication<'files, 'library> {
    NewOwned(PublishedFile<'files, 'library>),
    /// Reuse grants NO ownership of the existing bytes. The new private stage
    /// stays separately owned and can be explicitly discarded under custody.
    ReusedExisting {
        existing: ExistingFile<'files, 'library>,
        staged: OwnedStage<'files, 'library>,
    },
}

#[derive(Debug)]
pub(crate) struct MetadataStageCleanupFailure<'files, 'library> {
    pub(crate) cause: StorageError,
    // Keep the real file/directory witnesses even after a partial unlink or
    // lost flush acknowledgement. Cleanup cannot be retried from a pathname.
    pub(crate) publication: Publication<'files, 'library>,
}
impl<'files, 'library> Publication<'files, 'library> {
    /// Only the metadata owner calls this after a known SQL acknowledgement.
    /// Published/reused destination bytes are never removed; only the private
    /// empty container, and the separately owned unused payload on reuse, are.
    pub(crate) fn finish_metadata_stage(
        self,
        guard: &Custody<'_>,
    ) -> std::result::Result<(), Box<MetadataStageCleanupFailure<'files, 'library>>> {
        self.finish_metadata_stage_with(guard, Directory::flush)
    }
    fn finish_metadata_stage_with(
        mut self,
        guard: &Custody<'_>,
        flush: impl Fn(&Directory) -> Result<()>,
    ) -> std::result::Result<(), Box<MetadataStageCleanupFailure<'files, 'library>>> {
        let result = (|| {
            let verification = WorkCancellation::default();
            let stage = match &mut self {
                Self::NewOwned(published) => {
                    published.revalidate(guard, &verification)?;
                    &mut published.stage
                }
                Self::ReusedExisting { existing, staged } => {
                    existing.revalidate(guard, &verification)?;
                    staged.revalidate(guard)?;
                    if staged.directory.entries(1, &verification)? != [OsString::from(PAYLOAD)] {
                        return Err(StorageError::SnapshotChanged);
                    }
                    staged.revalidate(guard)?;
                    // No writes are admitted once cleanup has been entered,
                    // including when the unlink/flush acknowledgement is lost.
                    staged.published = true;
                    native::unlinkat(&staged.directory.file, PAYLOAD, AtFlags::empty()).map_err(
                        |e| StorageError::io("remove unused metadata payload", e.into()),
                    )?;
                    flush(&staged.directory)?;
                    staged
                }
            };
            stage.files.check(guard)?;
            stage
                .files
                .staging
                .require_name(&stage.directory_name, stage.directory.identity)?;
            stage.directory.entries(0, &verification)?;
            stage
                .files
                .staging
                .require_name(&stage.directory_name, stage.directory.identity)?;
            native::unlinkat(
                &stage.files.staging.file,
                &stage.directory_name,
                AtFlags::REMOVEDIR,
            )
            .map_err(|e| StorageError::io("remove empty metadata stage", e.into()))?;
            flush(&stage.files.staging)?;
            stage.files.check(guard)?;
            stage.forget_registration()
        })();
        if let Err(cause) = result {
            match &self {
                Self::NewOwned(published) => published.stage.files.fence(),
                Self::ReusedExisting { staged, .. } => staged.files.fence(),
            }
            return Err(Box::new(MetadataStageCleanupFailure {
                cause,
                publication: self,
            }));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PublishFailure<'files, 'library> {
    pub staged: OwnedStage<'files, 'library>,
    pub area: FileArea,
    pub name: EntryName,
    pub cause: StorageError,
    /// True if publication may have occurred, including unavailable native
    /// acknowledgement. Witnesses/bytes remain; further claims are fenced.
    pub publish_uncertain: bool,
}

/// Recoverable cleanup moves only an owned, unpublished private container to a
/// unique trash name. It does not purge bytes or claim DB-reference knowledge.
#[derive(Debug)]
pub struct RetiredTemp<'files, 'library> {
    staged: OwnedStage<'files, 'library>,
    name: EntryName,
}
impl RetiredTemp<'_, '_> {
    pub fn trash_name(&self) -> &EntryName {
        &self.name
    }
    pub fn revalidate(&self, guard: &Custody<'_>) -> Result<()> {
        self.staged.files.check(guard)?;
        self.staged
            .files
            .trash
            .require_name(self.name.as_os_str(), self.staged.directory.identity)?;
        require_regular(&self.staged.file)?;
        self.staged
            .directory
            .require_name(OsStr::new(PAYLOAD), self.staged.identity)
    }
}

#[derive(Debug)]
pub struct TempCleanupFailure<'files, 'library> {
    pub staged: OwnedStage<'files, 'library>,
    pub trash_name: Option<EntryName>,
    pub cause: StorageError,
    pub cleanup_uncertain: bool,
}

impl<'files, 'library> OwnedStage<'files, 'library> {
    /// Discard only the original unpublished private copy after the import
    /// owner has acknowledged a checksum-duplicate report. Existing managed
    /// bytes and picker sources are outside this capability.
    pub(crate) fn discard_duplicate_stage(
        mut self,
        guard: &Custody<'_>,
    ) -> std::result::Result<(), Box<TempCleanupFailure<'files, 'library>>> {
        let result = (|| {
            self.revalidate(guard)?;
            let cancel = WorkCancellation::default();
            if self.directory.entries(1, &cancel)? != [OsString::from(PAYLOAD)] {
                return Err(StorageError::SnapshotChanged);
            }
            self.revalidate(guard)?;
            self.published = true;
            native::unlinkat(&self.directory.file, PAYLOAD, AtFlags::empty())
                .map_err(|e| StorageError::io("discard duplicate private payload", e.into()))?;
            self.directory.flush()?;
            self.files
                .staging
                .require_name(&self.directory_name, self.directory.identity)?;
            self.directory.entries(0, &cancel)?;
            native::unlinkat(
                &self.files.staging.file,
                &self.directory_name,
                AtFlags::REMOVEDIR,
            )
            .map_err(|e| StorageError::io("discard duplicate private workspace", e.into()))?;
            self.files.staging.flush()?;
            self.files.check(guard)?;
            self.forget_registration()
        })();
        if let Err(cause) = result {
            self.files.fence();
            return Err(Box::new(TempCleanupFailure {
                staged: self,
                trash_name: None,
                cause,
                cleanup_uncertain: true,
            }));
        }
        Ok(())
    }
    pub fn inspector(&mut self) -> StageReader<'_> {
        StageReader {
            file: &mut self.file,
        }
    }
    pub fn measure(&mut self, limit: u64, cancellation: &WorkCancellation) -> Result<StreamDigest> {
        self.file
            .rewind()
            .map_err(|e| StorageError::io("rewind staged file", e))?;
        hash_stream(&mut self.file, limit, cancellation)
    }
    fn revalidate(&self, guard: &Custody<'_>) -> Result<()> {
        self.files.check(guard)?;
        self.revalidate_source()
    }
    fn revalidate_source(&self) -> Result<()> {
        if self.published || self.files.authority.fenced() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        self.files.revalidate()?;
        self.files
            .staging
            .require_name(&self.directory_name, self.directory.identity)?;
        require_regular(&self.file)?;
        self.directory
            .require_name(OsStr::new(PAYLOAD), self.identity)
    }

    pub fn publish_no_clobber(
        self,
        guard: &Custody<'_>,
        area: FileArea,
        name: EntryName,
        expected: StreamDigest,
        cancellation: &WorkCancellation,
    ) -> std::result::Result<Publication<'files, 'library>, Box<PublishFailure<'files, 'library>>>
    {
        self.publish_with_checkpoints(
            guard,
            area,
            name,
            expected,
            cancellation,
            (|| Ok(()), Directory::flush),
        )
    }

    fn publish_with_checkpoints(
        mut self,
        guard: &Custody<'_>,
        area: FileArea,
        name: EntryName,
        expected: StreamDigest,
        cancellation: &WorkCancellation,
        checkpoints: (
            impl FnOnce() -> Result<()>,
            impl Fn(&Directory) -> Result<()>,
        ),
    ) -> std::result::Result<Publication<'files, 'library>, Box<PublishFailure<'files, 'library>>>
    {
        let (before_rename, flush) = checkpoints;
        let before = (|| {
            self.revalidate(guard)?;
            durable_file_flush(&self.file)?;
            if self.measure(expected.bytes(), cancellation)? != expected {
                return Err(StorageError::SnapshotChanged);
            }
            self.revalidate(guard)?;
            before_rename()?;
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            Ok(())
        })();
        if let Err(cause) = before {
            return Err(Box::new(PublishFailure {
                staged: self,
                area,
                name,
                cause,
                publish_uncertain: false,
            }));
        }
        let destination_owner = self.files.clone();
        let destination = destination_owner.area(area);
        let rename = native::renameat_with(
            &self.directory.file,
            PAYLOAD,
            &destination.file,
            name.as_os_str(),
            RenameFlags::NOREPLACE,
        );
        if let Err(error) = rename {
            // A collision is the kernel's positive no-replace result. Treat
            // other errors conservatively as an unavailable acknowledgement;
            // no automatic source cleanup or destructive retry can follow.
            if error != rustix::io::Errno::EXIST {
                self.published = true;
                self.files.fence();
                return Err(Box::new(PublishFailure {
                    staged: self,
                    area,
                    name,
                    cause: StorageError::io("publish acknowledgement unavailable", error.into()),
                    publish_uncertain: true,
                }));
            }
            let existing = (|| {
                let mut file = destination.open_regular(name.as_os_str(), OFlags::RDONLY)?;
                let identity = FileIdentity::from_file(&file)?;
                if file
                    .metadata()
                    .map_err(|e| StorageError::io("inspect reuse size", e))?
                    .len()
                    != expected.bytes()
                {
                    return Err(StorageError::DestinationCollision);
                }
                let measured =
                    hash_stream(&mut file, expected.bytes(), cancellation).map_err(|error| {
                        match error {
                            StorageError::HashSizeLimit => StorageError::DestinationCollision,
                            error => error,
                        }
                    })?;
                if measured != expected {
                    return Err(StorageError::DestinationCollision);
                }
                require_regular(&file)?;
                destination.require_name(name.as_os_str(), identity)?;
                self.revalidate(guard)?;
                Ok(ExistingFile {
                    files: self.files.clone(),
                    area,
                    name: name.clone(),
                    file,
                    identity,
                    measurement: expected,
                })
            })();
            return match existing {
                Ok(existing) => Ok(Publication::ReusedExisting {
                    existing,
                    staged: self,
                }),
                Err(cause) => Err(Box::new(PublishFailure {
                    staged: self,
                    area,
                    name,
                    cause,
                    publish_uncertain: false,
                })),
            };
        }
        self.published = true;
        // From here cancellation is not a rollback or no-effect proof. Flush
        // both changed directories and the stage parent before acknowledging.
        let after = (|| {
            require_regular(&self.file)?;
            destination.require_name(name.as_os_str(), self.identity)?;
            if self.measure(expected.bytes(), &WorkCancellation::default())? != expected {
                return Err(StorageError::SnapshotChanged);
            }
            self.files.check(guard)?;
            flush(destination)?;
            flush(&self.directory)?;
            flush(&self.files.staging)?;
            destination.require_name(name.as_os_str(), self.identity)?;
            self.files.check(guard)
        })();
        if let Err(cause) = after {
            self.files.fence();
            return Err(Box::new(PublishFailure {
                staged: self,
                area,
                name,
                cause,
                publish_uncertain: true,
            }));
        }
        Ok(Publication::NewOwned(PublishedFile {
            stage: self,
            area,
            name,
            measurement: expected,
        }))
    }

    /// Return an unused owned stage to recoverable trash. This is NOT import
    /// compensation: it cannot operate on a published receipt, reused file or
    /// uncertainty. No foreign/substituted bytes are unlinked or overwritten.
    pub fn retire_owned_temp(
        self,
        guard: &Custody<'_>,
    ) -> std::result::Result<RetiredTemp<'files, 'library>, Box<TempCleanupFailure<'files, 'library>>>
    {
        self.retire_with_flush(guard, Directory::flush)
    }
    fn retire_with_flush(
        mut self,
        guard: &Custody<'_>,
        flush: impl Fn(&Directory) -> Result<()>,
    ) -> std::result::Result<RetiredTemp<'files, 'library>, Box<TempCleanupFailure<'files, 'library>>>
    {
        if let Err(cause) = self.revalidate(guard) {
            return Err(Box::new(TempCleanupFailure {
                staged: self,
                trash_name: None,
                cause,
                cleanup_uncertain: false,
            }));
        }
        if let Err(cause) = durable_file_flush(&self.file).and_then(|()| self.directory.flush()) {
            return Err(Box::new(TempCleanupFailure {
                staged: self,
                trash_name: None,
                cause,
                cleanup_uncertain: false,
            }));
        }
        let mut chosen = None;
        for _ in 0..64 {
            let name = unique_name("unused-stage");
            match native::renameat_with(
                &self.files.staging.file,
                &self.directory_name,
                &self.files.trash.file,
                &name,
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {
                    chosen = Some(EntryName(name));
                    break;
                }
                Err(error) if error == rustix::io::Errno::EXIST => continue,
                Err(error) => {
                    self.published = true;
                    self.files.fence();
                    return Err(Box::new(TempCleanupFailure {
                        staged: self,
                        trash_name: Some(EntryName(name)),
                        cause: StorageError::io("retire owned temporary container", error.into()),
                        cleanup_uncertain: true,
                    }));
                }
            }
        }
        let Some(name) = chosen else {
            return Err(Box::new(TempCleanupFailure {
                staged: self,
                trash_name: None,
                cause: StorageError::DestinationCollision,
                cleanup_uncertain: false,
            }));
        };
        // The retained file becomes read-only after its container moves, even
        // if durability/reporting fails. There is no destructive error Drop.
        self.published = true;
        let after = (|| {
            self.files
                .trash
                .require_name(name.as_os_str(), self.directory.identity)?;
            self.directory
                .require_name(OsStr::new(PAYLOAD), self.identity)?;
            require_regular(&self.file)?;
            flush(&self.files.trash)?;
            flush(&self.files.staging)?;
            self.files.check(guard)?;
            self.forget_registration()
        })();
        if let Err(cause) = after {
            self.files.fence();
            return Err(Box::new(TempCleanupFailure {
                staged: self,
                trash_name: Some(name),
                cause,
                cleanup_uncertain: true,
            }));
        }
        Ok(RetiredTemp { staged: self, name })
    }
}

impl LibraryFiles<'_> {
    /// Bounded read of a stored catalog-relative path. Every component is opened
    /// relative to retained directories with no-follow semantics, and every
    /// binding is checked again after reading. This grants no mutation rights.
    pub(crate) fn read_catalog(&self, path: &str, limit: usize) -> Result<Vec<u8>> {
        self.authority.require_intake()?;
        self.revalidate()?;
        let relative = path
            .strip_prefix("catalog/")
            .ok_or(StorageError::InvalidManagedPath)?;
        let parts = relative
            .split('/')
            .map(EntryName::parse)
            .collect::<Result<Vec<_>>>()?;
        if parts.is_empty() || parts.len() > 4 || limit > 10 * 1024 * 1024 {
            return Err(StorageError::InvalidManagedPath);
        }
        let mut directories = vec![self.catalog.duplicate()?];
        for name in &parts[..parts.len() - 1] {
            let parent = directories.last().ok_or(StorageError::InvalidManagedPath)?;
            directories.push(parent.child(name.as_os_str())?);
        }
        let parent = directories.last().ok_or(StorageError::InvalidManagedPath)?;
        let name = parts.last().ok_or(StorageError::InvalidManagedPath)?;
        let mut file = parent.open_regular(name.as_os_str(), OFlags::RDONLY)?;
        let identity = FileIdentity::from_file(&file)?;
        let before = file
            .metadata()
            .map_err(|e| StorageError::io("inspect catalog asset", e))?;
        if before.len() > limit as u64 {
            return Err(StorageError::HashSizeLimit);
        }
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read catalog asset", e))?;
        if bytes.len() > limit {
            return Err(StorageError::HashSizeLimit);
        }
        require_regular(&file)?;
        parent.require_name(name.as_os_str(), identity)?;
        for (index, name) in parts[..parts.len() - 1].iter().enumerate() {
            directories[index].require_name(name.as_os_str(), directories[index + 1].identity)?;
        }
        self.revalidate()?;
        let after = file
            .metadata()
            .map_err(|e| StorageError::io("reinspect catalog asset", e))?;
        if before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use librairii_core::LibraryId;
    use librairii_testkit::TestRoots;
    use std::{error::Error, fs, os::unix::fs::symlink};

    type TestResult = std::result::Result<(), Box<dyn Error>>;

    #[test]
    fn metadata_stage_cleanup_preserves_substituted_directories_and_payloads() -> TestResult {
        for reused in [false, true] {
            let roots = TestRoots::new()?;
            let library = isolated(roots.library())?;
            let files = LibraryFiles::open(&library)?;
            let guard = files.try_custody()?;
            let name = EntryName::parse("metadata.json")?;
            if reused {
                fs::write(
                    library.layout().catalog().join(name.as_os_str()),
                    b"catalog bytes",
                )?;
            }
            let (staged, measured) = stage(&files, &guard, b"catalog bytes")?;
            let original_identity = staged.identity;
            let private = library.layout().staging().join(&staged.directory_name);
            let publication = staged
                .publish_no_clobber(
                    &guard,
                    FileArea::Catalog,
                    name,
                    measured,
                    &WorkCancellation::default(),
                )
                .map_err(|f| f.cause)?;
            let foreign = if reused {
                fs::rename(private.join(PAYLOAD), private.join("held-original"))?;
                fs::write(private.join(PAYLOAD), b"foreign payload")?;
                private.join(PAYLOAD)
            } else {
                fs::rename(&private, library.layout().staging().join("held-original"))?;
                fs::create_dir(&private)?;
                fs::write(private.join("foreign"), b"foreign directory")?;
                private.join("foreign")
            };
            let before = fs::read(&foreign)?;
            let failure = publication
                .finish_metadata_stage(&guard)
                .err()
                .ok_or("substitution accepted")?;
            let retained_identity = match &failure.publication {
                Publication::NewOwned(published) => published.identity(),
                Publication::ReusedExisting { staged, .. } => staged.identity,
            };
            assert_eq!(retained_identity, original_identity);
            assert_eq!(fs::read(&foreign)?, before);
            assert_eq!(
                fs::read(library.layout().catalog().join("metadata.json"))?,
                b"catalog bytes"
            );
            drop(failure);
            drop(guard);
            assert!(files.try_custody().is_err());
        }
        Ok(())
    }

    #[test]
    fn metadata_stage_cleanup_flush_failure_keeps_actual_receipts_and_fences_custody() -> TestResult
    {
        for reused in [false, true] {
            let roots = TestRoots::new()?;
            let library = isolated(roots.library())?;
            let files = LibraryFiles::open(&library)?;
            let guard = files.try_custody()?;
            let destination = library.layout().catalog().join("metadata.json");
            if reused {
                fs::write(&destination, b"catalog bytes")?;
            }
            let (staged, measured) = stage(&files, &guard, b"catalog bytes")?;
            let original_identity = staged.identity;
            let private = library.layout().staging().join(&staged.directory_name);
            let publication = staged
                .publish_no_clobber(
                    &guard,
                    FileArea::Catalog,
                    EntryName::parse("metadata.json")?,
                    measured,
                    &WorkCancellation::default(),
                )
                .map_err(|f| f.cause)?;
            let failure = publication
                .finish_metadata_stage_with(&guard, |_| Err(StorageError::IntegrityFailed))
                .err()
                .ok_or("lost cleanup flush was success")?;
            let stage = match &failure.publication {
                Publication::NewOwned(published) => &published.stage,
                Publication::ReusedExisting { staged, .. } => staged,
            };
            assert_eq!(FileIdentity::from_file(&stage.file)?, original_identity);
            assert!(stage.published);
            assert_eq!(stage.file.metadata()?.len(), b"catalog bytes".len() as u64);
            assert_eq!(fs::read(&destination)?, b"catalog bytes");
            assert_eq!(private.exists(), reused);
            if reused {
                assert_eq!(fs::read_dir(private)?.count(), 0);
            }
            drop(failure);
            drop(guard);
            assert!(files.try_custody().is_err());
        }
        Ok(())
    }

    fn isolated(path: &Path) -> std::result::Result<IsolatedLibrary, Box<dyn Error>> {
        let identity: LibraryId = "a0000000-0000-4000-8000-000000000001".parse()?;
        Ok(IsolatedLibrary::initialize_from_override(
            Some(path.as_os_str()),
            identity,
        )?)
    }
    fn stage<'files, 'library>(
        files: &'files LibraryFiles<'library>,
        guard: &Custody<'_>,
        bytes: &[u8],
    ) -> Result<(OwnedStage<'files, 'library>, StreamDigest)> {
        let creation = files.create_stage(guard);
        assert!(creation.is_ok(), "successful-stage fixture: {creation:?}");
        let Ok(mut stage) = creation else {
            return Err(StorageError::CompatibilityInvariant);
        };
        stage
            .write_all(bytes)
            .map_err(|e| StorageError::io("write test stage", e))?;
        let measurement = stage.measure(bytes.len() as u64, &WorkCancellation::default())?;
        Ok((stage, measurement))
    }

    #[test]
    fn stage_creation_post_open_failure_preserves_bytes_and_fences_generation() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let mut observed = None;
        let creation = files.create_stage_after_open(&guard, |name, file| {
            let mut writer = file;
            writer
                .write_all(b"actual partial stage bytes")
                .map_err(|e| StorageError::io("write injected partial stage", e))?;
            observed = Some((name.to_owned(), FileIdentity::from_file(file)?));
            Err(StorageError::io(
                "inject stage creation failure after payload acquisition",
                io::Error::other("actual acquired descriptor"),
            ))
        });
        assert!(creation.is_err());
        let (name, identity) = observed.ok_or("payload was not actually acquired")?;
        let path = library.layout().staging().join(name).join(PAYLOAD);
        assert_eq!(fs::read(&path)?, b"actual partial stage bytes");
        let metadata = fs::metadata(&path)?;
        assert_eq!(metadata.dev(), identity.device);
        assert_eq!(metadata.ino(), identity.inode);
        assert!(
            library.custody_fenced.load(Ordering::Acquire),
            "post-acquisition failure lost its unresolved generation fence"
        );
        Ok(())
    }

    #[test]
    fn stage_creation_validation_faults_keep_every_acquired_handle_and_optional_identity()
    -> TestResult {
        for point in [
            StageCreatePoint::AfterMkdir,
            StageCreatePoint::DirectoryAcquired,
            StageCreatePoint::DirectoryType,
            StageCreatePoint::DirectoryIdentity,
            StageCreatePoint::DirectoryName,
            StageCreatePoint::BeforePayloadOpen,
            StageCreatePoint::PayloadAcquired,
            StageCreatePoint::PayloadType,
            StageCreatePoint::PayloadLinks,
            StageCreatePoint::PayloadIdentity,
            StageCreatePoint::PayloadName,
            StageCreatePoint::FinalCheck,
        ] {
            let roots = TestRoots::new()?;
            let library = isolated(roots.library())?;
            let files = LibraryFiles::open(&library)?;
            let guard = files.try_custody()?;
            let mut actual_directory = None;
            let mut actual_payload = None;
            let failure = files
                .create_stage_with(
                    &guard,
                    || unique_name("stage"),
                    |at, state| {
                        if at != point {
                            return Ok(());
                        }
                        actual_directory = state
                            .directory
                            .as_ref()
                            .map(FileIdentity::from_file)
                            .transpose()?;
                        if let Some(file) = state.payload.as_ref() {
                            let mut writer = file;
                            writer
                                .write_all(b"retained validation prefix")
                                .map_err(|e| StorageError::io("write fault prefix", e))?;
                            actual_payload = Some(FileIdentity::from_file(file)?);
                        }
                        Err(StorageError::io(
                            "stage validation fault",
                            io::Error::other("owned failure"),
                        ))
                    },
                )
                .err()
                .ok_or("fault did not return retained creation")?;
            assert_eq!(
                failure.disposition(),
                StageCreateDisposition::RetainedUnresolved
            );
            assert!(matches!(failure.cause(), StorageError::Io { .. }));
            let state = failure
                .retained_allocation()
                .ok_or("missing partial owner")?;
            assert!(std::ptr::eq(&*state.files, &files));
            assert_eq!(state.parent_identity(), files.staging.identity);
            assert!(state.mkdir_attempted());
            assert!(state.directory_created());
            assert_eq!(state.directory_acquired(), actual_directory.is_some());
            assert_eq!(state.payload_acquired(), actual_payload.is_some());
            assert_eq!(state.payload_created(), actual_payload.is_some());
            assert_eq!(state.payload_open_attempted(), actual_payload.is_some());
            assert_eq!(
                state
                    .directory
                    .as_ref()
                    .map(FileIdentity::from_file)
                    .transpose()?,
                actual_directory
            );
            assert_eq!(
                state
                    .payload
                    .as_ref()
                    .map(FileIdentity::from_file)
                    .transpose()?,
                actual_payload
            );
            let directory_observed = !matches!(
                point,
                StageCreatePoint::AfterMkdir
                    | StageCreatePoint::DirectoryAcquired
                    | StageCreatePoint::DirectoryType
                    | StageCreatePoint::DirectoryIdentity
            );
            let payload_observed = matches!(
                point,
                StageCreatePoint::PayloadName | StageCreatePoint::FinalCheck
            );
            assert_eq!(
                state.directory_identity(),
                if directory_observed {
                    actual_directory
                } else {
                    None
                }
            );
            assert_eq!(
                state.payload_identity(),
                if payload_observed {
                    actual_payload
                } else {
                    None
                }
            );
            let path = library.layout().staging().join(state.directory_name());
            assert!(path.is_dir());
            if state.payload_acquired() {
                let mut bytes = Vec::new();
                let mut reader = state.payload_file()?;
                reader.rewind()?;
                reader.read_to_end(&mut bytes)?;
                assert_eq!(bytes, b"retained validation prefix");
                assert_eq!(fs::read(path.join(state.payload_name()))?, bytes);
            }
            assert!(library.custody_fenced.load(Ordering::Acquire));
            // A later pre-effect refusal cannot acknowledge/clear this attempt.
            let later = files
                .create_stage(&guard)
                .err()
                .ok_or("earlier fence cleared")?;
            assert_eq!(later.disposition(), StageCreateDisposition::NoAllocation);
            assert!(matches!(
                later.cause(),
                StorageError::CustodyRecoveryRequired
            ));
            assert!(later.retained_allocation().is_none());
            assert!(library.custody_fenced.load(Ordering::Acquire));
            // No production cleanup occurs when negative test owners end.
            drop(later);
            drop(failure);
            assert!(path.is_dir());
            assert!(library.custody_fenced.load(Ordering::Acquire));
        }
        Ok(())
    }

    #[test]
    fn stage_creation_no_allocation_and_collisions_never_own_existing_bytes() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let pre = files
            .create_stage_with(
                &guard,
                || unique_name("stage"),
                |at, _| {
                    assert_eq!(at, StageCreatePoint::BeforeMkdir);
                    Err(StorageError::Cancelled)
                },
            )
            .err()
            .ok_or("pre-effect refusal missing")?;
        assert_eq!(pre.disposition(), StageCreateDisposition::NoAllocation);
        assert!(matches!(pre.cause(), StorageError::Cancelled));
        assert!(pre.retained_allocation().is_none());
        assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
        assert!(!library.custody_fenced.load(Ordering::Acquire));
        let occupied = library.layout().staging().join("occupied");
        fs::create_dir(&occupied)?;
        fs::write(occupied.join(PAYLOAD), b"foreign collision bytes")?;
        let mut attempts = 0;
        let collision = files
            .create_stage_with(
                &guard,
                || {
                    attempts += 1;
                    OsString::from("occupied")
                },
                |_, _| Ok(()),
            )
            .err()
            .ok_or("collision adopted existing stage")?;
        assert_eq!(attempts, 64);
        assert_eq!(
            collision.disposition(),
            StageCreateDisposition::NoAllocation
        );
        assert!(matches!(
            collision.cause(),
            StorageError::DestinationCollision
        ));
        assert!(collision.retained_allocation().is_none());
        assert!(!library.custody_fenced.load(Ordering::Acquire));
        assert_eq!(
            fs::read(occupied.join(PAYLOAD))?,
            b"foreign collision bytes"
        );
        assert_eq!(fs::read_dir(library.layout().staging())?.count(), 1);
        // Invalid single components refuse before mutation too.
        let invalid = files
            .create_stage_with(&guard, || OsString::from("../escape"), |_, _| Ok(()))
            .err()
            .ok_or("invalid component admitted")?;
        assert_eq!(invalid.disposition(), StageCreateDisposition::NoAllocation);
        assert!(matches!(invalid.cause(), StorageError::InvalidManagedPath));
        Ok(())
    }

    #[test]
    fn stage_creation_substitution_retains_original_handles_without_replacement_authority()
    -> TestResult {
        for target in [
            StageCreatePoint::DirectoryName,
            StageCreatePoint::PayloadName,
        ] {
            let roots = TestRoots::new()?;
            let library = isolated(roots.library())?;
            let files = LibraryFiles::open(&library)?;
            let guard = files.try_custody()?;
            let mut original = None;
            let mut replaced = false;
            let failure = files
                .create_stage_with(
                    &guard,
                    || unique_name("stage"),
                    |at, state| {
                        if at != target || replaced {
                            return Ok(());
                        }
                        replaced = true;
                        let path = library.layout().staging().join(state.directory_name());
                        if at == StageCreatePoint::DirectoryName {
                            original = Some(FileIdentity::from_file(state.directory_file()?)?);
                            fs::rename(&path, library.layout().staging().join("original-held"))
                                .map_err(|e| StorageError::io("test substitute directory", e))?;
                            fs::create_dir(&path)
                                .map_err(|e| StorageError::io("test replacement directory", e))?;
                            fs::write(path.join(PAYLOAD), b"replacement must survive")
                                .map_err(|e| StorageError::io("test replacement bytes", e))?;
                        } else {
                            original = Some(FileIdentity::from_file(state.payload_file()?)?);
                            let mut writer = state.payload_file()?;
                            writer
                                .write_all(b"original retained bytes")
                                .map_err(|e| StorageError::io("test original bytes", e))?;
                            fs::rename(path.join(PAYLOAD), path.join("original-held"))
                                .map_err(|e| StorageError::io("test substitute payload", e))?;
                            fs::write(path.join(PAYLOAD), b"replacement must survive")
                                .map_err(|e| StorageError::io("test replacement payload", e))?;
                        }
                        Ok(())
                    },
                )
                .err()
                .ok_or("substitution was accepted")?;
            assert!(matches!(failure.cause(), StorageError::SnapshotChanged));
            let state = failure
                .retained_allocation()
                .ok_or("substitution dropped partial owner")?;
            let retained_file = if target == StageCreatePoint::DirectoryName {
                state.directory_file()?
            } else {
                state.payload_file()?
            };
            assert_eq!(Some(FileIdentity::from_file(retained_file)?), original);
            let path = library.layout().staging().join(state.directory_name());
            assert_eq!(fs::read(path.join(PAYLOAD))?, b"replacement must survive");
            if target == StageCreatePoint::PayloadName {
                let mut reader = state.payload_file()?;
                reader.rewind()?;
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                assert_eq!(bytes, b"original retained bytes");
            } else {
                assert!(!state.payload_acquired());
            }
            assert!(library.custody_fenced.load(Ordering::Acquire));
            drop(failure);
            assert_eq!(fs::read(path.join(PAYLOAD))?, b"replacement must survive");
        }
        Ok(())
    }

    #[test]
    fn stage_creation_payload_link_change_after_metadata_never_returns_complete_authority()
    -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let failure = files
            .create_stage_with(
                &guard,
                || unique_name("stage"),
                |at, state| {
                    if at == StageCreatePoint::PayloadLinks {
                        let mut file = state.payload_file()?;
                        file.write_all(b"retained link-change bytes")
                            .map_err(|e| StorageError::io("test link-change bytes", e))?;
                        let path = library.layout().staging().join(state.directory_name());
                        fs::hard_link(path.join(state.payload_name()), path.join("extra-link"))
                            .map_err(|e| StorageError::io("test payload link change", e))?;
                    }
                    Ok(())
                },
            )
            .err()
            .ok_or("stale metadata granted complete stage authority")?;
        assert!(matches!(failure.cause(), StorageError::PathEscape));
        let state = failure
            .retained_allocation()
            .ok_or("hardlink failure lost owner")?;
        assert!(state.payload_created());
        assert_eq!(state.payload_file()?.metadata()?.nlink(), 2);
        assert_eq!(
            Some(FileIdentity::from_file(state.payload_file()?)?),
            state.payload_identity()
        );
        let path = library.layout().staging().join(state.directory_name());
        assert_eq!(
            fs::read(path.join("extra-link"))?,
            b"retained link-change bytes"
        );
        assert_eq!(
            fs::read(path.join(state.payload_name()))?,
            b"retained link-change bytes"
        );
        assert!(library.custody_fenced.load(Ordering::Acquire));
        Ok(())
    }

    #[test]
    fn stage_creation_syscall_errors_keep_attempt_truth_and_missing_handles() -> TestResult {
        for target in [
            StageCreatePoint::BeforeMkdir,
            StageCreatePoint::AfterMkdir,
            StageCreatePoint::BeforePayloadOpen,
        ] {
            let roots = TestRoots::new()?;
            let library = isolated(roots.library())?;
            let files = LibraryFiles::open(&library)?;
            let guard = files.try_custody()?;
            let foreign = library.layout().archives().join("unrelated");
            fs::create_dir(&foreign)?;
            fs::write(foreign.join(PAYLOAD), b"never adopt foreign bytes")?;
            let mut injected = false;
            let failure = files
                .create_stage_with(
                    &guard,
                    || unique_name("stage"),
                    |at, state| {
                        if at != target || injected {
                            return Ok(());
                        }
                        injected = true;
                        let path = library.layout().staging().join(state.directory_name());
                        match at {
                            StageCreatePoint::BeforeMkdir => {
                                fs::remove_dir(library.layout().staging())
                            }
                            StageCreatePoint::AfterMkdir => {
                                fs::remove_dir(&path).map_err(|e| {
                                    StorageError::io("test remove created directory", e)
                                })?;
                                symlink(&foreign, &path)
                            }
                            StageCreatePoint::BeforePayloadOpen => {
                                fs::write(path.join(PAYLOAD), b"never adopt collision bytes")
                            }
                            _ => return Err(StorageError::CompatibilityInvariant),
                        }
                        .map_err(|e| StorageError::io("test actual syscall failure", e))
                    },
                )
                .err()
                .ok_or("actual syscall refusal missing")?;
            assert_eq!(
                failure.disposition(),
                StageCreateDisposition::RetainedUnresolved
            );
            let state = failure
                .retained_allocation()
                .ok_or("attempt state missing")?;
            assert!(state.mkdir_attempted());
            assert_eq!(
                state.directory_created(),
                target != StageCreatePoint::BeforeMkdir
            );
            assert_eq!(
                state.directory_acquired(),
                target == StageCreatePoint::BeforePayloadOpen
            );
            assert_eq!(
                state.payload_open_attempted(),
                target == StageCreatePoint::BeforePayloadOpen
            );
            assert!(!state.payload_acquired());
            assert!(!state.payload_created());
            assert!(state.payload_identity().is_none());
            assert_eq!(
                fs::read(foreign.join(PAYLOAD))?,
                b"never adopt foreign bytes"
            );
            if target == StageCreatePoint::BeforePayloadOpen {
                assert_eq!(
                    fs::read(
                        library
                            .layout()
                            .staging()
                            .join(state.directory_name())
                            .join(PAYLOAD)
                    )?,
                    b"never adopt collision bytes"
                );
            }
            assert!(library.custody_fenced.load(Ordering::Acquire));
        }
        Ok(())
    }

    #[test]
    fn stage_creation_partial_native_owner_fences_finalization_and_preserves_active() -> TestResult
    {
        use crate::{
            adoption::{ClosedLibraryInput, ClosedManifest},
            platform::lock::LIBRARY_LOCK_FILE,
            profile::{NativeProfileStore, Profile, ProfileDocument, ProfileLocations},
        };
        let roots = TestRoots::new()?;
        let source = roots.library().join("closed-source");
        fs::create_dir(&source)?;
        let manifest_bytes =
            include_bytes!("../../../../preservation/corpus/profiles/library-manifest.json");
        let manifest: serde_json::Value = serde_json::from_slice(manifest_bytes)?;
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../preservation/corpus/libraries/schema018");
        for entry in manifest.as_array().ok_or("manifest array")? {
            let name = entry["path"].as_str().ok_or("manifest path")?;
            if entry["directory"].as_bool() == Some(true) {
                fs::create_dir(source.join(name))?;
            } else {
                fs::copy(corpus.join(name), source.join(name))?;
            }
        }
        let profile = Profile {
            version: 1,
            language: "fr".into(),
            root: source.to_str().ok_or("source UTF8")?.into(),
            library_id: "a0000000-0000-4000-8000-000000000001".into(),
            complete: true,
            recovering: false,
        };
        let cancel = WorkCancellation::default();
        let input = ClosedLibraryInput::verify(
            &source,
            ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
            ClosedManifest::decode(manifest_bytes)?,
            &cancel,
        )?;
        let locations = ProfileLocations::for_host(
            &roots.profile().join("home"),
            Some(&roots.profile().join("config")),
            Some(&roots.library().join("data")),
        )?;
        fs::create_dir_all(&locations.data)?;
        fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
        let copy = input.copy_to_new_root(&locations.data.join("actual-copy"), &cancel)?;
        let root = copy.root().to_owned();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &cancel)
            .map_err(|failure| failure.to_string())?;
        let mut db = Database::open_native(&native)?;
        let partial_path;
        {
            let files = LibraryFiles::open_native(&db)?;
            let guard = files.try_custody()?;
            let retained = files
                .create_stage_after_open(&guard, |_, file| {
                    let mut writer = file;
                    writer
                        .write_all(b"native unresolved allocation")
                        .map_err(|e| StorageError::io("write native partial bytes", e))?;
                    Err(StorageError::io(
                        "injected native creation validation",
                        io::Error::other("keep owners"),
                    ))
                })
                .err()
                .ok_or("native failure lost")?;
            let state = retained
                .retained_allocation()
                .ok_or("native partial owner absent")?;
            partial_path = root
                .join("staging")
                .join(state.directory_name())
                .join(state.payload_name());
            assert_eq!(fs::read(&partial_path)?, b"native unresolved allocation");
            assert!(files.try_custody().is_err());
            // Mandatory SAME-writer observation survives the generation fence.
            assert_eq!(
                db.barrier_read(
                    |tx| Ok(tx.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?)
                )?
                .wait()?,
                1
            );
            // This negative fixture ends borrows; it does NOT resolve bytes or
            // grant settlement. The native fence must survive these drops.
            drop(retained);
        }
        db.close()?;
        drop(db);
        let retained_native = native
            .finish()
            .err()
            .ok_or("partial allocation minted Closed")?;
        assert_eq!(fs::read(&partial_path)?, b"native unresolved allocation");
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(crate::native::NATIVE_STATE_FILE))?)?;
        assert_eq!(record["state"], "active");
        assert!(store.update(|_| Ok(())).is_err());
        let probe = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(LIBRARY_LOCK_FILE))?;
        assert!(matches!(
            probe.try_lock(),
            Err(fs::TryLockError::WouldBlock)
        ));
        assert!(matches!(
            NativeProfileStore::open_native(&locations.native),
            Err(StorageError::ProfileInUse)
        ));
        assert_eq!(retained_native.retained_library().layout().root(), root);
        Ok(())
    }

    #[test]
    fn owned_read_source_retains_readonly_fd_raw_name_and_cooperative_observations() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let name = EntryName::parse("Éte # %2F retained.bin")?;
        let path = library.layout().archives().join(name.as_os_str());
        std::fs::write(&path, b"stable source bytes")?;
        let files = LibraryFiles::open(&library)?;
        let mut source = files.open_read_source(FileArea::Archives, name.clone())?;
        assert_eq!(source.name(), &name);
        assert_eq!(source.initial().bytes(), 19);
        assert_eq!(source.initial().links(), 1);
        assert_eq!(source.observe()?, source.initial());
        assert_eq!(source.revalidate()?, source.initial());
        assert_eq!(
            native::fcntl_getfl(&source.file)? & OFlags::ACCMODE,
            OFlags::RDONLY
        );
        let mut attempted_write = &source.file;
        assert!(attempted_write.write_all(b"must not write").is_err());
        let cancel = WorkCancellation::default();
        let limit = source.initial().bytes();
        let before = hash_stream(&mut source, limit, &cancel)?;
        source.rewind()?;
        assert_eq!(hash_stream(&mut source, limit, &cancel)?, before);
        assert_eq!(source.revalidate()?, source.initial());
        library.custody_fenced.store(true, Ordering::Release);
        assert!(matches!(
            files.open_read_source(FileArea::Archives, name),
            Err(StorageError::CustodyRecoveryRequired)
        ));
        // Retained readonly observations do not require new effect admission,
        // and do not clear the sticky fence.
        source.rewind()?;
        assert_eq!(hash_stream(&mut source, limit, &cancel)?, before);
        assert!(library.custody_fenced.load(Ordering::Acquire));
        assert_eq!(std::fs::read(&path)?, b"stable source bytes");
        Ok(())
    }

    #[test]
    fn owned_read_source_refuses_aliases_replaced_names_and_partial_components() -> TestResult {
        use std::os::unix::fs::symlink;
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let archives = library.layout().archives();
        let path = archives.join("owned");
        std::fs::write(&path, b"original readonly bytes")?;
        let files = LibraryFiles::open(&library)?;
        for raw in ["", ".", "..", "a/b", "/absolute", "nul\0tail"] {
            assert!(EntryName::parse(raw).is_err());
        }
        assert!(
            files
                .open_read_source(FileArea::Archives, EntryName::parse("missing")?)
                .is_err()
        );
        assert!(!archives.join("missing").exists());
        std::fs::create_dir(archives.join("directory"))?;
        assert!(
            files
                .open_read_source(FileArea::Archives, EntryName::parse("directory")?)
                .is_err()
        );
        let alias = archives.join("alias");
        std::fs::hard_link(&path, &alias)?;
        assert!(matches!(
            files.open_read_source(FileArea::Archives, EntryName::parse("alias")?),
            Err(StorageError::PathEscape)
        ));
        std::fs::remove_file(&alias)?;
        let outside = roots.profile().join("foreign");
        std::fs::write(&outside, b"foreign unchanged")?;
        symlink(&outside, &alias)?;
        assert!(
            files
                .open_read_source(FileArea::Archives, EntryName::parse("alias")?)
                .is_err()
        );
        let mut source = files.open_read_source(FileArea::Archives, EntryName::parse("owned")?)?;
        std::fs::rename(&path, archives.join("displaced"))?;
        std::fs::write(&path, b"replacement unchanged")?;
        assert!(source.observe().is_err());
        assert!(source.read(&mut [0]).is_err());
        // The real old FD survives detected substitution; no cleanup/reopen
        // capability is exposed to the consumer or inferred from a pathname.
        let mut retained = Vec::new();
        source.file.read_to_end(&mut retained)?;
        assert_eq!(retained, b"original readonly bytes");
        assert_eq!(std::fs::read(&path)?, b"replacement unchanged");
        assert_eq!(std::fs::read(&outside)?, b"foreign unchanged");
        Ok(())
    }

    #[test]
    fn owned_read_source_detects_parent_substitution_and_keeps_actual_owner() -> TestResult {
        use std::os::unix::fs::symlink;
        for alias_parent in [false, true] {
            let roots = TestRoots::new()?;
            let library = isolated(roots.library())?;
            let archives = library.layout().archives();
            std::fs::write(archives.join("owned"), b"retained original")?;
            let files = LibraryFiles::open(&library)?;
            let mut source =
                files.open_read_source(FileArea::Archives, EntryName::parse("owned")?)?;
            let original_parent = roots.library().join("displaced-archives");
            std::fs::rename(&archives, &original_parent)?;
            if alias_parent {
                symlink(&original_parent, &archives)?;
            } else {
                std::fs::create_dir(&archives)?;
                std::fs::write(archives.join("owned"), b"foreign replacement")?;
            }
            assert!(source.revalidate().is_err());
            assert!(source.seek(SeekFrom::Start(0)).is_err());
            assert!(
                files
                    .open_read_source(FileArea::Archives, EntryName::parse("owned")?)
                    .is_err()
            );
            let mut retained = Vec::new();
            source.file.read_to_end(&mut retained)?;
            assert_eq!(retained, b"retained original");
            assert_eq!(
                std::fs::read(original_parent.join("owned"))?,
                b"retained original"
            );
        }
        Ok(())
    }

    #[test]
    fn owned_read_source_reports_size_changes_and_never_claims_snapshot_exclusion() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let path = library.layout().catalog().join("source");
        std::fs::write(&path, b"initial")?;
        let files = LibraryFiles::open(&library)?;
        let mut source = files.open_read_source(FileArea::Catalog, EntryName::parse("source")?)?;
        std::fs::write(&path, b"cooperatively observed changed bytes")?;
        let observed = source.observe()?;
        assert_eq!(observed.identity(), source.initial().identity());
        assert_ne!(observed.bytes(), source.initial().bytes());
        assert!(matches!(
            source.revalidate(),
            Err(StorageError::SnapshotChanged)
        ));
        assert!(source.read(&mut [0; 4]).is_err());
        assert_eq!(
            std::fs::read(&path)?,
            b"cooperatively observed changed bytes"
        );
        Ok(())
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn owned_read_source_retains_non_utf8_native_basename_without_reencoding() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let raw = OsString::from_vec(b"raw-\xff-#-%2F.bin".to_vec());
        let name = EntryName::parse(raw.clone())?;
        let path = library.layout().archives().join(&raw);
        std::fs::write(&path, b"lossless native source")?;
        let files = LibraryFiles::open(&library)?;
        let mut source = files.open_read_source(FileArea::Archives, name)?;
        assert_eq!(source.name().as_os_str().as_bytes(), raw.as_bytes());
        let mut bytes = Vec::new();
        source.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"lossless native source");
        assert_eq!(source.revalidate()?, source.initial());
        assert_eq!(std::fs::read(&path)?, b"lossless native source");
        Ok(())
    }

    #[test]
    fn owned_control_creation_failure_keeps_actual_fd_name_and_partial_bytes() -> TestResult {
        let roots = TestRoots::new()?;
        let directory = Directory::open(roots.library())?;
        let name = OsStr::new(".librairii-native-state.json.tmp-owned-failure");
        let failure = directory
            .create_control_with(name, |file| {
                let mut file = file;
                file.write_all(b"partial owned control bytes")
                    .map_err(|e| StorageError::io("write actual control prefix", e))?;
                Err(StorageError::io(
                    "injected post-open control validation",
                    io::Error::other("retain actual fd"),
                ))
            })
            .err()
            .ok_or("post-open failure lost")?;
        assert_eq!(failure.allocation.name, name);
        let file = failure
            .allocation
            .file
            .as_ref()
            .ok_or("missing retained actual fd")?;
        let expected = FileIdentity::from_file(file)?;
        directory.require_name(name, expected)?;
        assert_eq!(
            std::fs::read(roots.library().join(name))?,
            b"partial owned control bytes"
        );
        drop(failure);
        assert_eq!(
            std::fs::read(roots.library().join(name))?,
            b"partial owned control bytes"
        );
        Ok(())
    }

    #[test]
    fn descriptor_custody_is_shared_across_instances_and_bound_to_actual_generation() -> TestResult
    {
        fn require_send<T: Send>() {}
        require_send::<Custody<'_>>();
        let roots = TestRoots::new()?;
        let library = isolated(&roots.library().join("first"))?;
        let other = isolated(&roots.library().join("same-uuid-other-generation"))?;
        let files = LibraryFiles::open(&library)?;
        let peer = LibraryFiles::open(&library)?;
        let other_files = LibraryFiles::open(&other)?;
        let guard = files.try_custody()?;
        assert!(matches!(peer.try_custody(), Err(StorageError::CustodyBusy)));
        let wrong_generation = other_files
            .create_stage(&guard)
            .err()
            .ok_or("foreign generation admitted")?;
        assert!(matches!(
            wrong_generation.cause(),
            StorageError::IdentityMismatch
        ));
        assert_eq!(
            wrong_generation.disposition(),
            StageCreateDisposition::NoAllocation
        );
        drop(guard);
        drop(peer.try_custody()?);
        for name in ["", ".", "..", "a/b", "a\0b", "/abs"] {
            assert!(EntryName::parse(name).is_err());
        }
        assert_eq!(
            EntryName::parse("É # %2F.7z")?.as_os_str(),
            OsStr::new("É # %2F.7z")
        );
        let held = library.layout().root().join("saved-archives");
        fs::rename(library.layout().archives(), &held)?;
        fs::create_dir(library.layout().archives())?;
        assert!(matches!(
            LibraryFiles::open(&library),
            Err(StorageError::SnapshotChanged)
        ));
        fs::remove_dir(library.layout().archives())?;
        fs::rename(&held, library.layout().archives())?;
        drop(files.try_custody()?);
        Ok(())
    }

    #[test]
    fn native_no_clobber_reuses_exact_regular_bytes_without_gaining_cleanup_ownership() -> TestResult
    {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let name = EntryName::parse("raw-É # %2F.pack")?;
        let (new, measured) = stage(&files, &guard, b"independent complete bytes")?;
        let Publication::NewOwned(mut receipt) = new
            .publish_no_clobber(
                &guard,
                FileArea::Archives,
                name.clone(),
                measured,
                &WorkCancellation::default(),
            )
            .map_err(|e| e.cause)?
        else {
            return Err("expected owned publication".into());
        };
        receipt.revalidate(&guard, &WorkCancellation::default())?;
        let identity = receipt.identity();
        let (second, measured) = stage(&files, &guard, b"independent complete bytes")?;
        let Publication::ReusedExisting {
            mut existing,
            staged,
        } = second
            .publish_no_clobber(
                &guard,
                FileArea::Archives,
                name.clone(),
                measured,
                &WorkCancellation::default(),
            )
            .map_err(|e| e.cause)?
        else {
            return Err("expected validated reuse".into());
        };
        assert_eq!(existing.identity(), identity);
        existing.revalidate(&guard, &WorkCancellation::default())?;
        let retired = staged.retire_owned_temp(&guard).map_err(|e| e.cause)?;
        retired.revalidate(&guard)?;
        assert_eq!(
            fs::read(
                library
                    .layout()
                    .trash()
                    .join(retired.trash_name().as_os_str())
                    .join(PAYLOAD)
            )?,
            b"independent complete bytes"
        );
        drop(retired);
        drop(existing);
        drop(receipt);
        assert_eq!(
            fs::read(library.layout().archives().join(name.as_os_str()))?,
            b"independent complete bytes"
        );
        assert_eq!(fs::read_dir(library.layout().trash())?.count(), 1);
        Ok(())
    }

    #[test]
    fn destination_created_after_preflight_is_never_overwritten_by_publication() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let destination = library.layout().archives().join("race.pack");
        let (staged, measured) = stage(&files, &guard, b"new archive")?;
        let failure = staged
            .publish_with_checkpoints(
                &guard,
                FileArea::Archives,
                EntryName::parse("race.pack")?,
                measured,
                &WorkCancellation::default(),
                (
                    || {
                        fs::write(&destination, b"racing owner")
                            .map_err(|e| StorageError::io("inject destination race", e))
                    },
                    Directory::flush,
                ),
            )
            .err()
            .ok_or("racing destination must collide")?;
        assert!(!failure.publish_uncertain);
        assert!(matches!(failure.cause, StorageError::DestinationCollision));
        assert_eq!(fs::read(&destination)?, b"racing owner");
        failure
            .staged
            .retire_owned_temp(&guard)
            .map_err(|e| e.cause)?;
        assert_eq!(fs::read(&destination)?, b"racing owner");
        Ok(())
    }

    #[test]
    fn destination_aliases_and_same_inode_changed_bytes_do_not_pass_receipt_checks() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let foreign = roots.profile().join("foreign");
        fs::write(&foreign, b"original")?;
        symlink(&foreign, library.layout().archives().join("linked"))?;
        fs::hard_link(&foreign, library.layout().archives().join("aliased"))?;
        for name in ["linked", "aliased"] {
            let (staged, measured) = stage(&files, &guard, b"original")?;
            let failure = staged
                .publish_no_clobber(
                    &guard,
                    FileArea::Archives,
                    EntryName::parse(name)?,
                    measured,
                    &WorkCancellation::default(),
                )
                .err()
                .ok_or("aliases cannot be reused")?;
            assert!(!failure.publish_uncertain);
            drop(failure);
        }
        assert_eq!(fs::read(&foreign)?, b"original");
        let (staged, measured) = stage(&files, &guard, b"original")?;
        let Publication::NewOwned(mut receipt) = staged
            .publish_no_clobber(
                &guard,
                FileArea::Catalog,
                EntryName::parse("owned")?,
                measured,
                &WorkCancellation::default(),
            )
            .map_err(|e| e.cause)?
        else {
            return Err("expected publication".into());
        };
        fs::write(library.layout().catalog().join("owned"), b"modified")?;
        assert!(matches!(
            receipt.revalidate(&guard, &WorkCancellation::default()),
            Err(StorageError::SnapshotChanged)
        ));
        Ok(())
    }

    #[test]
    fn anchored_destination_survives_parent_substitution_without_touching_outside_root()
    -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let mut database = crate::database::Database::open_isolated(&library)?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let old = library.layout().root().join("retained-archives");
        let (staged, measured) = stage(&files, &guard, b"owned bytes")?;
        let failure = staged
            .publish_with_checkpoints(
                &guard,
                FileArea::Archives,
                EntryName::parse("redirect.pack")?,
                measured,
                &WorkCancellation::default(),
                (
                    || {
                        fs::rename(library.layout().archives(), &old)
                            .and_then(|()| symlink(roots.profile(), library.layout().archives()))
                            .map_err(|e| StorageError::io("inject parent substitution", e))
                    },
                    Directory::flush,
                ),
            )
            .err()
            .ok_or("namespace substitution must not receive durable success")?;
        assert!(failure.publish_uncertain);
        assert!(!roots.profile().join("redirect.pack").exists());
        assert_eq!(fs::read(old.join("redirect.pack"))?, b"owned bytes");
        drop(failure);
        drop(guard);
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyRecoveryRequired)
        ));
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
        Ok(())
    }

    #[test]
    fn publish_flush_failure_retains_witnesses_readonly_bytes_and_sticky_custody_fence()
    -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let mut database = crate::database::Database::open_isolated(&library)?;
        let (entered, wait_entered) = std::sync::mpsc::sync_channel(1);
        let (release, wait_release) = std::sync::mpsc::sync_channel(1);
        let critical = database.repair_transaction(move |_| {
            entered
                .send(())
                .map_err(|e| StorageError::io("signal critical entry", io::Error::other(e)))?;
            wait_release
                .recv_timeout(std::time::Duration::from_secs(10))
                .map_err(|e| {
                    StorageError::io("wait bounded critical release", io::Error::other(e))
                })?;
            Ok(())
        })?;
        wait_entered.recv_timeout(std::time::Duration::from_secs(10))?;
        let executed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = executed.clone();
        let queued = database.transaction(WorkCancellation::default(), move |_| {
            observed.store(true, Ordering::Release);
            Ok(())
        })?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let (staged, measured) = stage(&files, &guard, b"already renamed")?;
        let mut failure = staged
            .publish_with_checkpoints(
                &guard,
                FileArea::Archives,
                EntryName::parse("durability.pack")?,
                measured,
                &WorkCancellation::default(),
                (
                    || Ok(()),
                    |_| {
                        Err(StorageError::io(
                            "inject flush failure",
                            io::Error::other("lost durability acknowledgement"),
                        ))
                    },
                ),
            )
            .err()
            .ok_or("post-rename failure is uncertain")?;
        assert!(failure.publish_uncertain);
        assert!(
            failure
                .staged
                .write_all(b"never mutate published bytes")
                .is_err()
        );
        let mut readback = Vec::new();
        failure.staged.inspector().seek(SeekFrom::Start(0))?;
        failure.staged.inspector().read_to_end(&mut readback)?;
        assert_eq!(readback, b"already renamed");
        drop(failure);
        drop(guard);
        assert_eq!(
            fs::read(library.layout().archives().join("durability.pack"))?,
            b"already renamed"
        );
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyRecoveryRequired)
        ));
        release.send(())?;
        assert!(matches!(
            critical.wait()?,
            crate::database::TransactionOutcome::Committed(())
        ));
        assert!(matches!(
            queued.wait()?,
            crate::database::TransactionOutcome::ProvenNotCommitted(StorageError::RuntimeFenced)
        ));
        assert!(!executed.load(Ordering::Acquire));
        assert!(matches!(
            database.transaction(WorkCancellation::default(), |_| Ok(())),
            Err(StorageError::CustodyRecoveryRequired)
        ));
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
        Ok(())
    }

    #[test]
    fn cancellation_before_publish_has_no_effect_but_cancel_after_rename_retains_success()
    -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let (staged, measured) = stage(&files, &guard, b"first")?;
        let cancel = WorkCancellation::default();
        cancel.cancel();
        let failure = staged
            .publish_no_clobber(
                &guard,
                FileArea::Archives,
                EntryName::parse("before")?,
                measured,
                &cancel,
            )
            .err()
            .ok_or("pre-publication cancellation must refuse")?;
        assert!(!failure.publish_uncertain);
        assert!(matches!(failure.cause, StorageError::Cancelled));
        assert!(!library.layout().archives().join("before").exists());
        failure
            .staged
            .retire_owned_temp(&guard)
            .map_err(|e| e.cause)?;
        let (staged, measured) = stage(&files, &guard, b"second")?;
        let late = WorkCancellation::default();
        let published = staged
            .publish_with_checkpoints(
                &guard,
                FileArea::Archives,
                EntryName::parse("after")?,
                measured,
                &late,
                (
                    || Ok(()),
                    |dir| {
                        late.cancel();
                        dir.flush()
                    },
                ),
            )
            .map_err(|e| e.cause)?;
        assert!(matches!(published, Publication::NewOwned(_)));
        assert!(late.is_cancelled());
        assert_eq!(
            fs::read(library.layout().archives().join("after"))?,
            b"second"
        );
        Ok(())
    }

    #[test]
    fn changed_stage_or_cleanup_container_never_grants_substituted_file_ownership() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let (mut staged, measured) = stage(&files, &guard, b"same-size")?;
        staged.file.rewind()?;
        staged.write_all(b"different")?;
        let failure = staged
            .publish_no_clobber(
                &guard,
                FileArea::Archives,
                EntryName::parse("changed")?,
                measured,
                &WorkCancellation::default(),
            )
            .err()
            .ok_or("final rehash must detect changed bytes")?;
        assert!(matches!(failure.cause, StorageError::SnapshotChanged));
        assert!(!library.layout().archives().join("changed").exists());
        let path = library
            .layout()
            .staging()
            .join(&failure.staged.directory_name);
        let retained = library.layout().staging().join("retained-original-stage");
        fs::rename(&path, &retained)?;
        fs::create_dir(&path)?;
        fs::write(path.join(PAYLOAD), b"foreign must remain")?;
        let cleanup = failure
            .staged
            .retire_owned_temp(&guard)
            .err()
            .ok_or("replacement is not owned cleanup")?;
        assert!(!cleanup.cleanup_uncertain);
        drop(cleanup);
        assert_eq!(fs::read(path.join(PAYLOAD))?, b"foreign must remain");
        assert_eq!(fs::read(retained.join(PAYLOAD))?, b"different");
        assert_eq!(fs::read_dir(library.layout().trash())?.count(), 0);
        Ok(())
    }

    #[test]
    fn atomic_config_exchange_preserves_a_racing_foreign_config_instead_of_overwriting_it()
    -> TestResult {
        let roots = TestRoots::new()?;
        let directory = Directory::open(roots.profile())?;
        let path = roots.profile().join("settings.json");
        fs::write(&path, b"previous owned config")?;
        let expected = fs::metadata(&path)?;
        let mut temporary = directory.create_config_temp()?;
        temporary.file.write_all(b"new owned config")?;
        let displaced_name = temporary.name.clone();
        let failure = directory
            .replace_config_with_checkpoint(
                temporary,
                OsStr::new("settings.json"),
                Some(&expected),
                || {
                    fs::rename(&path, roots.profile().join("retained-original"))
                        .and_then(|()| fs::write(&path, b"racing foreign config"))
                        .map_err(|e| StorageError::io("inject config race", e))
                },
            )
            .err()
            .ok_or("raced config exchange must report changed uncertainty")?;
        assert!(failure.changed);
        assert!(matches!(failure.cause, StorageError::SnapshotChanged));
        assert_eq!(fs::read(&path)?, b"new owned config");
        drop(failure);
        assert_eq!(
            fs::read(roots.profile().join(displaced_name))?,
            b"racing foreign config"
        );
        assert_eq!(
            fs::read(roots.profile().join("retained-original"))?,
            b"previous owned config"
        );
        Ok(())
    }

    #[test]
    fn absent_config_uses_no_replace_and_owned_temp_cleanup_refuses_substituted_names() -> TestResult
    {
        let roots = TestRoots::new()?;
        let directory = Directory::open(roots.profile())?;
        let path = roots.profile().join("settings.json");
        let mut temporary = directory.create_config_temp()?;
        temporary.file.write_all(b"new owned config")?;
        let failure = directory
            .replace_config_with_checkpoint(temporary, OsStr::new("settings.json"), None, || {
                fs::write(&path, b"racing foreign config")
                    .map_err(|e| StorageError::io("inject first config race", e))
            })
            .err()
            .ok_or("racing first config cannot be overwritten")?;
        assert!(!failure.changed);
        let temporary_path = roots.profile().join(&failure.temporary.name);
        fs::rename(&temporary_path, roots.profile().join("retained-owned-temp"))?;
        fs::write(&temporary_path, b"foreign temporary replacement")?;
        assert!(matches!(
            directory.remove_config_temp(&failure.temporary),
            Err(StorageError::SnapshotChanged)
        ));
        drop(failure);
        assert_eq!(fs::read(&path)?, b"racing foreign config");
        assert_eq!(fs::read(&temporary_path)?, b"foreign temporary replacement");
        assert_eq!(
            fs::read(roots.profile().join("retained-owned-temp"))?,
            b"new owned config"
        );
        Ok(())
    }

    #[test]
    fn source_substitution_after_hash_never_receives_an_owned_published_receipt() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let (staged, measured) = stage(&files, &guard, b"originally owned")?;
        let directory = library.layout().staging().join(&staged.directory_name);
        let mut failure = staged
            .publish_with_checkpoints(
                &guard,
                FileArea::Archives,
                EntryName::parse("race.pack")?,
                measured,
                &WorkCancellation::default(),
                (
                    || {
                        fs::rename(directory.join(PAYLOAD), directory.join("retained-original"))
                            .and_then(|()| {
                                fs::write(directory.join(PAYLOAD), b"substituted object")
                            })
                            .map_err(|e| StorageError::io("inject source substitution", e))
                    },
                    Directory::flush,
                ),
            )
            .err()
            .ok_or("substitution must remain uncertain")?;
        assert!(failure.publish_uncertain);
        assert!(matches!(failure.cause, StorageError::SnapshotChanged));
        assert!(failure.staged.write_all(b"no mutation").is_err());
        assert_eq!(
            failure
                .staged
                .measure(measured.bytes(), &WorkCancellation::default())?,
            measured
        );
        drop(failure);
        assert_eq!(
            fs::read(directory.join("retained-original"))?,
            b"originally owned"
        );
        assert_eq!(
            fs::read(library.layout().archives().join("race.pack"))?,
            b"substituted object"
        );
        drop(guard);
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyRecoveryRequired)
        ));
        Ok(())
    }

    #[test]
    fn unused_temp_trash_flush_failure_preserves_bytes_and_both_location_witnesses() -> TestResult {
        let roots = TestRoots::new()?;
        let library = isolated(roots.library())?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let (staged, _) = stage(&files, &guard, b"owned recovery bytes")?;
        let source = library.layout().staging().join(&staged.directory_name);
        let mut failure = staged
            .retire_with_flush(&guard, |_| {
                Err(StorageError::io(
                    "inject trash directory flush",
                    io::Error::other("lost acknowledgement"),
                ))
            })
            .err()
            .ok_or("trash publication needs uncertainty")?;
        assert!(failure.cleanup_uncertain);
        assert!(!source.exists());
        let trash_name = failure.trash_name.as_ref().ok_or("missing trash witness")?;
        assert_eq!(
            fs::read(
                library
                    .layout()
                    .trash()
                    .join(trash_name.as_os_str())
                    .join(PAYLOAD)
            )?,
            b"owned recovery bytes"
        );
        assert!(failure.staged.write_all(b"no mutation").is_err());
        drop(failure);
        drop(guard);
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyRecoveryRequired)
        ));
        Ok(())
    }
}

impl OwnedStage<'_, '_> {
    /// Register this actual retained allocation in the durable native control
    /// record before writing payload bytes. A scope DTO cannot construct a stage.
    /// Errors leave this original owner in the caller; never delete by basename.
    pub fn register_operation(
        &mut self,
        guard: &Custody<'_>,
        operation: String,
        item: i64,
    ) -> Result<()> {
        self.revalidate(guard)?;
        let FilesAuthority::Native { library, database } = self.files.authority else {
            return Err(StorageError::IdentityMismatch);
        };
        let (scope, _, _) = database.operation_temp_scope(operation.clone(), item, true)?;
        durable_file_flush(&self.file)?;
        self.directory.flush()?;
        self.files.staging.flush()?;
        let registration = crate::native_effects::Temporary {
            operation,
            item,
            scope_sha256: scope,
            destination: None,
            parent: self.files.staging.identity(),
            name: self
                .directory_name
                .to_str()
                .ok_or(StorageError::InvalidManagedPath)?
                .into(),
            directory: Some(self.directory.identity()),
            payload: Some(self.identity),
        };
        if self
            .registration
            .as_ref()
            .is_some_and(|prior| prior.as_ref() != &registration)
        {
            return Err(StorageError::SnapshotChanged);
        }
        // Keep the exact registration with the actual owner even if its durable
        // update loses acknowledgement. Publication moves this owner intact.
        self.registration = Some(Box::new(registration.clone()));
        library.register_temporary(registration)
    }
    fn forget_registration(&mut self) -> Result<()> {
        if let Some(registration) = &self.registration {
            let FilesAuthority::Native { library, .. } = self.files.authority else {
                return Err(StorageError::IdentityMismatch);
            };
            library.forget_temporary(registration)?;
            self.registration = None;
        }
        Ok(())
    }
}
impl LibraryFiles<'_> {
    pub(crate) fn require_empty_staging(&self) -> Result<()> {
        if !self
            .staging
            .entries(0, &WorkCancellation::default())?
            .is_empty()
        {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        Ok(())
    }
    pub(crate) fn recover_registered_temporary(
        &self,
        guard: &Custody<'_>,
        record: &crate::native_effects::Temporary,
    ) -> Result<()> {
        self.check(guard)?;
        self.retire_registered_record(record)
    }
    fn retire_registered_record(&self, record: &crate::native_effects::Temporary) -> Result<()> {
        record.validate()?;
        let parent = match &record.destination {
            None => self.staging.duplicate()?,
            Some(path) => {
                if std::fs::canonicalize(path)
                    .map_err(|e| StorageError::io("resolve registered export directory", e))?
                    != Path::new(path)
                {
                    return Err(StorageError::SnapshotChanged);
                }
                Directory::open(Path::new(path))?
            }
        };
        if parent.identity() != record.parent {
            return Err(StorageError::SnapshotChanged);
        }
        let directory = match parent.child(OsStr::new(&record.name)) {
            Ok(d) => d,
            Err(StorageError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                return parent.flush();
            }
            Err(error) => return Err(error),
        };
        if Some(directory.identity()) != record.directory {
            return Err(StorageError::SnapshotChanged);
        }
        for name in directory.entries(1, &WorkCancellation::default())? {
            if name != OsStr::new(PAYLOAD) {
                return Err(StorageError::CustodyRecoveryRequired);
            }
        }
        if let Some(file) = directory.read_regular(OsStr::new(PAYLOAD))? {
            if FileIdentity::from_file(&file)?
                != record
                    .payload
                    .ok_or(StorageError::CustodyRecoveryRequired)?
            {
                return Err(StorageError::SnapshotChanged);
            }
            directory.require_name(
                OsStr::new(PAYLOAD),
                record
                    .payload
                    .ok_or(StorageError::CustodyRecoveryRequired)?,
            )?;
            native::unlinkat(&directory.file, PAYLOAD, AtFlags::empty())
                .map_err(|e| StorageError::io("remove registered regular temporary", e.into()))?;
            directory.flush()?;
        }
        parent.require_name(
            OsStr::new(&record.name),
            record
                .directory
                .ok_or(StorageError::CustodyRecoveryRequired)?,
        )?;
        native::unlinkat(&parent.file, OsStr::new(&record.name), AtFlags::REMOVEDIR)
            .map_err(|e| StorageError::io("remove registered temporary workspace", e.into()))?;
        parent.flush()
    }
}

/// Allocation retains the actual native library, external directory and every
/// acquired FD through partial creation or an uncertain registration result.
#[derive(Debug)]
pub(crate) struct ExportAllocation<'files, 'library> {
    files: &'files LibraryFiles<'library>,
    parent: Option<Directory>,
    directory: Option<Directory>,
    file: Option<File>,
    name: OsString,
    record: Option<crate::native_effects::Temporary>,
    final_name: Option<OsString>,
    uncertain: bool,
    workspace_retired: bool,
}
#[derive(Debug)]
pub struct ExportTemporaryFailure<'files, 'library> {
    pub cause: StorageError,
    allocation: ExportAllocation<'files, 'library>,
}
impl std::fmt::Display for ExportTemporaryFailure<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "owned export temporary: {}", self.cause)
    }
}
impl std::error::Error for ExportTemporaryFailure<'_, '_> {}
impl ExportTemporaryFailure<'_, '_> {
    pub fn workspace_name(&self) -> &OsStr {
        &self.allocation.name
    }
}
#[derive(Debug)]
pub struct OwnedExportTemporary<'files, 'library> {
    allocation: ExportAllocation<'files, 'library>,
}
impl<'library> LibraryFiles<'library> {
    /// Creates only inside this real running export's registered destination.
    /// Scope/path values alone cannot create or clean up this actual owner.
    pub fn create_export_temporary<'files>(
        &'files self,
        guard: &Custody<'_>,
        operation: String,
        item: i64,
    ) -> std::result::Result<
        OwnedExportTemporary<'files, 'library>,
        Box<ExportTemporaryFailure<'files, 'library>>,
    > {
        self.create_export_temporary_with(guard, operation, item, || Ok(()))
    }
    fn create_export_temporary_with<'files>(
        &'files self,
        guard: &Custody<'_>,
        operation: String,
        item: i64,
        after_acquire: impl FnOnce() -> Result<()>,
    ) -> std::result::Result<
        OwnedExportTemporary<'files, 'library>,
        Box<ExportTemporaryFailure<'files, 'library>>,
    > {
        let mut allocation = ExportAllocation {
            files: self,
            parent: None,
            directory: None,
            file: None,
            name: OsString::new(),
            record: None,
            final_name: None,
            uncertain: false,
            workspace_retired: false,
        };
        let result = (|| {
            self.check(guard)?;
            let FilesAuthority::Native { library, database } = self.authority else {
                return Err(StorageError::IdentityMismatch);
            };
            let (scope, destination, kind) =
                database.operation_temp_scope(operation.clone(), item, true)?;
            if kind != librairii_core::OperationKind::Export {
                return Err(StorageError::SnapshotChanged);
            }
            let path = destination.ok_or(StorageError::InvalidManagedPath)?;
            if path.as_os_str().len() > 16 * 1024
                || std::fs::canonicalize(&path)
                    .map_err(|e| StorageError::io("resolve chosen export directory", e))?
                    != Path::new(&path)
            {
                return Err(StorageError::InvalidManagedPath);
            }
            allocation.parent = Some(Directory::open(Path::new(&path))?);
            let parent = allocation
                .parent
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            for _ in 0..64 {
                allocation.name = unique_name("export");
                let reservation = crate::native_effects::Temporary {
                    operation: operation.clone(),
                    item,
                    scope_sha256: scope.clone(),
                    destination: Some(path.clone()),
                    parent: parent.identity(),
                    name: allocation
                        .name
                        .to_str()
                        .ok_or(StorageError::InvalidManagedPath)?
                        .into(),
                    directory: None,
                    payload: None,
                };
                allocation.record = Some(reservation.clone());
                library.register_temporary(reservation)?;
                match parent.create_child(&allocation.name) {
                    Ok(directory) => {
                        allocation.directory = Some(directory);
                        break;
                    }
                    Err(StorageError::Io { source, .. })
                        if source.kind() == io::ErrorKind::AlreadyExists =>
                    {
                        library.forget_temporary(
                            allocation
                                .record
                                .as_ref()
                                .ok_or(StorageError::CompatibilityInvariant)?,
                        )?;
                        allocation.record = None;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            let directory = allocation
                .directory
                .as_ref()
                .ok_or(StorageError::QueueFull)?;
            allocation.file = Some(match directory.create_control_record(OsStr::new(PAYLOAD)) {
                Ok(control) => control.file,
                Err(failure) => {
                    let mut failure = *failure;
                    allocation.file = failure.allocation.file.take();
                    return Err(failure.cause);
                }
            });
            let file = allocation
                .file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            after_acquire()?;
            durable_file_flush(file)?;
            directory.flush()?;
            parent.flush()?;
            let record = crate::native_effects::Temporary {
                operation,
                item,
                scope_sha256: scope,
                destination: Some(path),
                parent: parent.identity(),
                name: allocation
                    .name
                    .to_str()
                    .ok_or(StorageError::InvalidManagedPath)?
                    .into(),
                directory: Some(directory.identity()),
                payload: Some(FileIdentity::from_file(file)?),
            };
            allocation.record = Some(record.clone());
            library.register_temporary(record)
        })();
        match result {
            Ok(()) => Ok(OwnedExportTemporary { allocation }),
            Err(cause) => {
                if allocation.record.is_some()
                    || allocation.directory.is_some()
                    || allocation.file.is_some()
                {
                    self.fence();
                }
                Err(Box::new(ExportTemporaryFailure { cause, allocation }))
            }
        }
    }
}
impl<'files, 'library> OwnedExportTemporary<'files, 'library> {
    pub fn workspace_name(&self) -> &OsStr {
        &self.allocation.name
    }
    fn revalidate(&self) -> Result<()> {
        self.allocation.files.authority.require_intake()?;
        let record = self
            .allocation
            .record
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let parent = self
            .allocation
            .parent
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let directory = self
            .allocation
            .directory
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let file = self
            .allocation
            .file
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        require_regular(file)?;
        if self.allocation.final_name.is_some() || self.allocation.uncertain {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        if let Some(path) = &record.destination {
            parent.matches_metadata(
                &std::fs::symlink_metadata(path)
                    .map_err(|e| StorageError::io("revalidate export destination", e))?,
            )?;
        }
        parent.require_name(
            &self.allocation.name,
            record
                .directory
                .ok_or(StorageError::CustodyRecoveryRequired)?,
        )?;
        directory.require_name(
            OsStr::new(PAYLOAD),
            record
                .payload
                .ok_or(StorageError::CustodyRecoveryRequired)?,
        )?;
        Ok(())
    }
    pub fn flush_durable(&self) -> Result<()> {
        self.revalidate()?;
        durable_file_flush(
            self.allocation
                .file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?,
        )?;
        self.allocation
            .directory
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .flush()
    }
    /// This can retire only the registered private workspace, never a final
    /// destination filename. Failure returns the original allocation owner.
    pub fn retire(
        self,
        guard: &Custody<'_>,
    ) -> std::result::Result<(), Box<ExportTemporaryFailure<'files, 'library>>> {
        let result = (|| {
            let FilesAuthority::Native { library, .. } = self.allocation.files.authority else {
                return Err(StorageError::IdentityMismatch);
            };
            let record = self
                .allocation
                .record
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            self.allocation
                .files
                .recover_registered_temporary(guard, record)?;
            library.forget_temporary(record)
        })();
        match result {
            Ok(()) => Ok(()),
            Err(cause) => {
                self.allocation.files.fence();
                Err(Box::new(ExportTemporaryFailure {
                    cause,
                    allocation: self.allocation,
                }))
            }
        }
    }
}
impl Write for OwnedExportTemporary<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.revalidate().map_err(io::Error::other)?;
        self.allocation
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("missing actual temporary"))?
            .write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.allocation
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("missing actual temporary"))?
            .flush()
    }
}

impl LibraryFiles<'_> {
    fn compensate_export_allocation(
        &self,
        guard: &Custody<'_>,
        allocation: &ExportAllocation<'_, '_>,
    ) -> Result<()> {
        if !self.authority.same_generation(guard.authority) || !std::ptr::eq(self, allocation.files)
        {
            return Err(StorageError::IdentityMismatch);
        }
        let FilesAuthority::Native { library, .. } = self.authority else {
            return Err(StorageError::IdentityMismatch);
        };
        library.revalidate_database_ownership()?;
        self.retire_registered_record(
            allocation
                .record
                .as_ref()
                .ok_or(StorageError::CustodyRecoveryRequired)?,
        )
    }
}
impl Directory {
    pub(crate) fn retire_control_allocation(&self, allocation: &ControlAllocation) -> Result<()> {
        if let Some(file) = &allocation.file {
            use std::os::unix::fs::MetadataExt;
            let metadata = file
                .metadata()
                .map_err(|e| StorageError::io("observe retained control allocation", e))?;
            if metadata.nlink() == 0 {
                return Ok(());
            }
            require_regular(file)?;
            self.require_name(&allocation.name, FileIdentity::from_file(file)?)?;
            native::unlinkat(&self.file, &allocation.name, AtFlags::empty())
                .map_err(|e| StorageError::io("retire actual control allocation", e.into()))?;
        }
        self.flush()
    }
}
