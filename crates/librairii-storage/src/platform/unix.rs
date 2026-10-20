//! Descriptor-relative mechanisms for the explicit fresh library authority.
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
use crate::{IsolatedLibrary, Result, StorageError, database::WorkCancellation};
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
    sync::atomic::{AtomicU64, Ordering},
};

const PAYLOAD: &str = "payload";
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    device: u64,
    inode: u64,
}
impl FileIdentity {
    fn from_file(file: &File) -> Result<Self> {
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

/// Private replace-owned-config capability. NativeProfileStore alone invokes
/// it under its retained namespace OS lock and serialized state. It has no
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

impl Directory {
    pub(crate) fn create_config_temp(&self) -> Result<ConfigTemp> {
        for _ in 0..64 {
            let name = unique_name("settings");
            match native::openat(
                &self.file,
                &name,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(fd) => {
                    let file = File::from(fd);
                    require_regular(&file)?;
                    let identity = FileIdentity::from_file(&file)?;
                    self.require_name(&name, identity)?;
                    return Ok(ConfigTemp {
                        file,
                        name,
                        identity,
                    });
                }
                Err(error) if error == rustix::io::Errno::EXIST => continue,
                Err(error) => {
                    return Err(StorageError::io(
                        "create owned config temporary",
                        error.into(),
                    ));
                }
            }
        }
        Err(StorageError::DestinationCollision)
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

/// Library generation is represented by an actual borrow of the authority, not
/// its UUID. Multiple filesystem handles share the SAME all-claimant guard.
///
/// The real lease cannot close while a stage still owns open resources:
/// ```compile_fail
/// use librairii_storage::{IsolatedLibrary, Result, platform::unix::LibraryFiles};
/// fn release_early(library: IsolatedLibrary) -> Result<()> {
///     let files = LibraryFiles::open(&library)?;
///     let guard = files.try_custody()?;
///     let staged = files.create_stage(&guard)?;
///     drop(guard);
///     drop(files); // the retained stage still borrows the directory owners
///     drop(library);
///     drop(staged);
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct LibraryFiles<'library> {
    authority: &'library IsolatedLibrary,
    root: Directory,
    staging: Directory,
    archives: Directory,
    catalog: Directory,
    trash: Directory,
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
    authority: &'library IsolatedLibrary,
}
impl Drop for Custody<'_> {
    fn drop(&mut self) {
        self.authority
            .custody_active
            .store(false, Ordering::Release);
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
        let files = Self {
            staging: root.child(OsStr::new("staging"))?,
            archives: root.child(OsStr::new("archives"))?,
            catalog: root.child(OsStr::new("catalog"))?,
            trash: root.child(OsStr::new("trash"))?,
            root,
            authority,
        };
        files.revalidate()?;
        Ok(files)
    }
    pub fn try_custody(&self) -> Result<Custody<'library>> {
        if self.authority.custody_fenced.load(Ordering::Acquire) {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        self.authority
            .custody_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| StorageError::CustodyBusy)?;
        let guard = Custody {
            authority: self.authority,
        };
        self.check(&guard)?;
        Ok(guard)
    }
    fn check(&self, guard: &Custody<'_>) -> Result<()> {
        if !std::ptr::eq(self.authority, guard.authority) {
            return Err(StorageError::IdentityMismatch);
        }
        if self.authority.custody_fenced.load(Ordering::Acquire) {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        self.revalidate()
    }
    fn revalidate(&self) -> Result<()> {
        self.authority.revalidate_ownership()?;
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
        Ok(())
    }
    fn fence(&self) {
        self.authority.custody_fenced.store(true, Ordering::Release);
    }
    fn area(&self, area: FileArea) -> &Directory {
        match area {
            FileArea::Archives => &self.archives,
            FileArea::Catalog => &self.catalog,
        }
    }
    pub fn create_stage(&self, guard: &Custody<'_>) -> Result<OwnedStage<'_, 'library>> {
        self.check(guard)?;
        for _ in 0..64 {
            let name = unique_name("stage");
            match native::mkdirat(&self.staging.file, &name, Mode::RWXU) {
                Ok(()) => {
                    let directory = self.staging.child(&name)?;
                    let file = directory.open_regular(
                        OsStr::new(PAYLOAD),
                        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL,
                    )?;
                    let identity = FileIdentity::from_file(&file)?;
                    self.check(guard)?;
                    return Ok(OwnedStage {
                        files: self,
                        directory,
                        directory_name: name,
                        file,
                        identity,
                        published: false,
                    });
                }
                Err(error) if error == rustix::io::Errno::EXIST => continue,
                Err(error) => {
                    return Err(StorageError::io(
                        "create exclusive stage directory",
                        error.into(),
                    ));
                }
            }
        }
        Err(StorageError::DestinationCollision)
    }
}

/// This handle owns an exclusively created regular file and private directory.
/// Drop closes handles and PRESERVES bytes. No pathname/hash tuple can create it.
#[derive(Debug)]
pub struct OwnedStage<'files, 'library> {
    files: &'files LibraryFiles<'library>,
    directory: Directory,
    directory_name: OsString,
    file: File,
    identity: FileIdentity,
    published: bool,
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
    files: &'files LibraryFiles<'library>,
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
        if self.published || self.files.authority.custody_fenced.load(Ordering::Acquire) {
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
        let destination = self.files.area(area);
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
                    files: self.files,
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
            self.files.check(guard)
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

#[cfg(test)]
mod tests {
    use super::*;
    use librairii_core::LibraryId;
    use librairii_testkit::TestRoots;
    use std::{error::Error, fs, os::unix::fs::symlink};

    type TestResult = std::result::Result<(), Box<dyn Error>>;
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
        let mut stage = files.create_stage(guard)?;
        stage
            .write_all(bytes)
            .map_err(|e| StorageError::io("write test stage", e))?;
        let measurement = stage.measure(bytes.len() as u64, &WorkCancellation::default())?;
        Ok((stage, measurement))
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
        assert!(matches!(
            other_files.create_stage(&guard),
            Err(StorageError::IdentityMismatch)
        ));
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
