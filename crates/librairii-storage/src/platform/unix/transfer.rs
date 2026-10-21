//! Explicitly selected external directory. It confers no library write authority.
use super::*;
use std::path::PathBuf;
#[derive(Debug)]
pub(crate) struct Destination {
    pub(super) directory: Directory,
    path: PathBuf,
}
impl Destination {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(librairii_core::export::ExportError::Destination.into());
        }
        let path = std::fs::canonicalize(path)
            .map_err(|e| StorageError::io("resolve selected destination", e))?;
        let result = Self {
            directory: Directory::open(&path)?,
            path,
        };
        result.revalidate()?;
        Ok(result)
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn label(&self) -> OsString {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_owned()
    }
    pub(crate) fn revalidate(&self) -> Result<()> {
        let metadata = std::fs::symlink_metadata(&self.path)
            .map_err(|e| StorageError::io("revalidate selected destination", e))?;
        self.directory.matches_metadata(&metadata)
    }
    pub(crate) fn exists(&self, name: &str) -> Result<bool> {
        let name = EntryName::parse(name)?;
        match native::statat(
            &self.directory.file,
            name.as_os_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(_) => Ok(true),
            Err(e) if e == rustix::io::Errno::NOENT => Ok(false),
            Err(e) => Err(StorageError::io("inspect export collision", e.into())),
        }
    }
}

/// Partial acquisition always returns the actual retained directory/file. No
/// destructive Drop; a failed acknowledgement never becomes cleanup authority.
#[derive(Debug)]
pub(crate) struct Output {
    directory: Directory,
    path: PathBuf,
    name: OsString,
    file: Option<File>,
    identity: Option<FileIdentity>,
    pub(crate) published: bool,
    pub(crate) uncertain: bool,
    attempted: Option<OsString>,
}
#[derive(Debug)]
pub(crate) struct OutputFailure {
    pub(crate) output: Option<Output>,
    pub(crate) cause: StorageError,
}
impl Destination {
    pub(crate) fn create_output(&self) -> std::result::Result<Output, Box<OutputFailure>> {
        // Before attempting creation no ownership has transferred. Duplicate
        // acquisition errors have no filesystem effects.
        let directory = match self.directory.duplicate() {
            Ok(d) => d,
            Err(cause) => {
                return Err(Box::new(OutputFailure {
                    output: None,
                    cause,
                }));
            }
        };
        let mut out = Output {
            directory,
            path: self.path.clone(),
            name: unique_name("export"),
            file: None,
            identity: None,
            published: false,
            uncertain: false,
            attempted: None,
        };
        let result = (|| {
            self.revalidate()?;
            let fd = native::openat(
                &out.directory.file,
                &out.name,
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|e| StorageError::io("create selected output temporary", e.into()))?;
            out.file = Some(File::from(fd));
            let file = out
                .file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            require_regular(file)?;
            out.identity = Some(FileIdentity::from_file(file)?);
            out.validate()?;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(out),
            Err(cause) => Err(Box::new(OutputFailure {
                output: Some(out),
                cause,
            })),
        }
    }
}
impl Output {
    pub(crate) fn stream(
        &mut self,
        source: &mut impl Read,
        expected: librairii_core::Checksum,
        bytes: u64,
        cancel: &WorkCancellation,
        mut progress: impl FnMut(u64),
    ) -> Result<()> {
        self.validate()?;
        struct CopyReader<'a, R, F> {
            source: &'a mut R,
            file: &'a mut File,
            progress: F,
            bytes: u64,
        }
        impl<R: Read, F: FnMut(u64)> Read for CopyReader<'_, R, F> {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let count = self.source.read(buffer)?;
                self.file.write_all(&buffer[..count])?;
                self.bytes += count as u64;
                (self.progress)(self.bytes);
                Ok(count)
            }
        }
        {
            let file = self
                .file
                .as_mut()
                .ok_or(StorageError::CustodyRecoveryRequired)?;
            let actual = hash_stream(
                &mut CopyReader {
                    source,
                    file,
                    progress: &mut progress,
                    bytes: 0,
                },
                bytes,
                cancel,
            )?;
            if actual.bytes() != bytes || actual.checksum() != expected {
                return Err(StorageError::SnapshotChanged);
            }
            durable_file_flush(file)?;
            file.rewind()
                .map_err(|e| StorageError::io("rewind copied output", e))?;
            let readback = hash_stream(file, bytes, cancel)?;
            if readback != actual {
                return Err(StorageError::SnapshotChanged);
            }
        }
        self.validate()
    }
    pub(crate) fn publish(&mut self, name: &str) -> Result<()> {
        self.validate()?;
        let name = EntryName::parse(name)?;
        self.attempted = Some(name.as_os_str().to_owned());
        self.uncertain = true;
        match native::renameat_with(
            &self.directory.file,
            &self.name,
            &self.directory.file,
            name.as_os_str(),
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {
                self.name = name.as_os_str().to_owned();
                self.published = true;
            }
            Err(e) if e == rustix::io::Errno::EXIST => {
                self.uncertain = false;
                return Err(StorageError::DestinationCollision);
            }
            Err(e) => {
                return Err(StorageError::io(
                    "publish output without replacement",
                    e.into(),
                ));
            }
        }
        self.validate()?;
        self.directory.flush()?;
        self.uncertain = false;
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        self.directory.matches_metadata(
            &std::fs::symlink_metadata(&self.path)
                .map_err(|e| StorageError::io("recheck output directory", e))?,
        )?;
        let file = self
            .file
            .as_ref()
            .ok_or(StorageError::CustodyRecoveryRequired)?;
        require_regular(file)?;
        let identity = self.identity.ok_or(StorageError::CustodyRecoveryRequired)?;
        if FileIdentity::from_file(file)? != identity {
            return Err(StorageError::SnapshotChanged);
        }
        self.directory.require_name(&self.name, identity)
    }
    pub(crate) fn cleanup(&mut self) -> Result<()> {
        if self.published || self.uncertain {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        // An openat failure before acquisition never owns an existing name.
        if self.file.is_none() {
            return Ok(());
        }
        self.validate()?;
        native::unlinkat(&self.directory.file, &self.name, AtFlags::empty())
            .map_err(|e| StorageError::io("remove owned output temporary", e.into()))?;
        self.uncertain = true;
        self.directory.flush()?;
        self.file = None;
        self.uncertain = false;
        Ok(())
    }
}

/// Retains every no-follow parent of a nested legacy managed archive, under the
/// actual Database/library borrow. No supplied path/identity grants mutation.
#[derive(Debug)]
pub(crate) struct ArchiveSource<'a, 'library> {
    files: &'a LibraryFiles<'library>,
    directories: Vec<(EntryName, Directory)>,
    file: File,
    name: EntryName,
    initial: ReadSourceObservation,
}
impl<'a, 'library> ArchiveSource<'a, 'library> {
    pub(crate) fn open(files: &'a LibraryFiles<'library>, relative: &str) -> Result<Self> {
        files.authority.require_intake()?;
        files.revalidate()?;
        let mut parts = relative.split('/');
        if parts.next() != Some("archives") {
            return Err(StorageError::PathEscape);
        }
        let mut names = parts.map(EntryName::parse).collect::<Result<Vec<_>>>()?;
        let name = names.pop().ok_or(StorageError::PathEscape)?;
        let mut directories = vec![(EntryName::parse("archives")?, files.archives.duplicate()?)];
        for name in names {
            let directory = directories
                .last()
                .ok_or(StorageError::PathEscape)?
                .1
                .child(name.as_os_str())?;
            directories.push((name, directory));
        }
        let parent = &directories.last().ok_or(StorageError::PathEscape)?.1;
        let file = parent.open_regular(name.as_os_str(), OFlags::RDONLY)?;
        let stat = file
            .metadata()
            .map_err(|e| StorageError::io("observe archive source", e))?;
        let initial = ReadSourceObservation {
            parent: parent.identity,
            file: FileIdentity::from_file(&file)?,
            bytes: stat.len(),
            links: stat.nlink(),
            modified: (stat.mtime(), stat.mtime_nsec()),
            changed: (stat.ctime(), stat.ctime_nsec()),
        };
        let source = Self {
            files,
            directories,
            file,
            name,
            initial,
        };
        source.validate()?;
        Ok(source)
    }
    pub(crate) fn bytes(&self) -> u64 {
        self.initial.bytes
    }
    pub(crate) fn validate(&self) -> Result<()> {
        self.files.revalidate()?;
        let mut parent = &self.files.root;
        for (name, directory) in &self.directories {
            parent.require_name(name.as_os_str(), directory.identity)?;
            parent = directory;
        }
        require_regular(&self.file)?;
        parent.require_name(self.name.as_os_str(), self.initial.file)?;
        let s = self
            .file
            .metadata()
            .map_err(|e| StorageError::io("revalidate archive source", e))?;
        if s.len() != self.initial.bytes
            || (s.mtime(), s.mtime_nsec()) != self.initial.modified
            || (s.ctime(), s.ctime_nsec()) != self.initial.changed
        {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(())
    }
}
impl Read for ArchiveSource<'_, '_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.validate().map_err(io::Error::other)?;
        self.file.read(buffer)
    }
}
