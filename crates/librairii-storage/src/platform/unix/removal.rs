//! Retained managed/trash parents and the actual archive descriptor. No paths
//! supplied by a caller confer deletion authority; removal never unlinks bytes.
use super::*;
use crate::removal::Scope;

#[derive(Debug)]
pub(crate) struct RemovalFile<'files, 'library> {
    files: &'files LibraryFiles<'library>,
    managed: Vec<(EntryName, Directory)>,
    trash: Option<Vec<(EntryName, Directory)>>,
    trash_path: std::path::PathBuf,
    managed_name: EntryName,
    trash_name: EntryName,
    file: File,
    identity: FileIdentity,
}
impl LibraryFiles<'_> {
    pub(crate) fn matches_database(&self, database: &Database<'_>) -> Result<()> {
        let actual = database.metadata_files()?;
        if !self.authority.same_generation(actual.authority) {
            return Err(StorageError::IdentityMismatch);
        }
        Ok(())
    }
    pub(super) fn drain_ownership(&self) -> Result<()> {
        match self.authority {
            FilesAuthority::Isolated(library) => library.revalidate_database_ownership(),
            FilesAuthority::Native { library, .. } => library.revalidate_database_ownership(),
        }
    }
}
fn parents(
    files: &LibraryFiles<'_>,
    path: &Path,
    area: &str,
    create: bool,
) -> Result<(Vec<(EntryName, Directory)>, EntryName)> {
    if path.as_os_str().len() > 4096 {
        return Err(StorageError::InvalidManagedPath);
    }
    let mut parts = path.as_os_str().as_bytes().split(|b| *b == b'/');
    if parts.next() != Some(area.as_bytes()) {
        return Err(StorageError::PathEscape);
    }
    let mut names = parts
        .map(|p| EntryName::parse(OsStr::from_bytes(p)))
        .collect::<Result<Vec<_>>>()?;
    if names.len() > 64 {
        return Err(StorageError::InvalidManagedPath);
    }
    let name = names.pop().ok_or(StorageError::PathEscape)?;
    let base = if area == "archives" {
        &files.archives
    } else {
        &files.trash
    };
    let mut retained = vec![(EntryName::parse(area)?, base.duplicate()?)];
    for name in names {
        let parent = &retained.last().ok_or(StorageError::PathEscape)?.1;
        let directory = match parent.child(name.as_os_str()) {
            Ok(d) => d,
            Err(StorageError::Io { source, .. })
                if create && source.kind() == io::ErrorKind::NotFound =>
            {
                let child = parent.create_child(name.as_os_str())?;
                child.flush()?;
                parent.flush()?;
                child
            }
            Err(e) => return Err(e),
        };
        retained.push((name, directory));
    }
    Ok((retained, name))
}
fn named(parent: &Directory, name: &EntryName) -> Result<Option<FileIdentity>> {
    match native::statat(&parent.file, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Ok(Some(FileIdentity::from_stat(&stat))),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(e) => Err(StorageError::io("observe removal location", e.into())),
    }
}
impl<'files, 'library> RemovalFile<'files, 'library> {
    pub(crate) fn open(
        files: &'files LibraryFiles<'library>,
        guard: &Custody<'_>,
        scope: &Scope,
        trash: &Path,
        recovery: bool,
    ) -> Result<Self> {
        files.check(guard)?;
        let (managed, managed_name) = parents(files, &scope.managed, "archives", recovery)?;
        let trash_path = trash.to_path_buf();
        let (trash, trash_name) = match parents(files, trash, "trash", false) {
            Ok((chain, name)) => (Some(chain), name),
            Err(StorageError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => (
                None,
                EntryName::parse(trash.file_name().ok_or(StorageError::PathEscape)?)?,
            ),
            Err(e) => return Err(e),
        };
        let m = &managed.last().ok_or(StorageError::PathEscape)?.1;
        let observed_trash = match &trash {
            Some(chain) => named(
                &chain.last().ok_or(StorageError::PathEscape)?.1,
                &trash_name,
            )?,
            None => None,
        };
        let file = match (named(m, &managed_name)?, observed_trash) {
            (Some(_), None) => m.open_regular(managed_name.as_os_str(), OFlags::RDONLY)?,
            (None, Some(_)) if recovery => trash
                .as_ref()
                .and_then(|c| c.last())
                .ok_or(StorageError::PathEscape)?
                .1
                .open_regular(trash_name.as_os_str(), OFlags::RDONLY)?,
            _ => return Err(StorageError::CustodyRecoveryRequired),
        };
        let actual = Self {
            files,
            managed,
            trash,
            trash_path,
            managed_name,
            trash_name,
            identity: FileIdentity::from_file(&file)?,
            file,
        };
        actual.location()?;
        Ok(actual)
    }
    fn parent_chain(&self, chain: &[(EntryName, Directory)]) -> Result<()> {
        self.files.drain_ownership()?;
        let mut parent = &self.files.root;
        for (name, directory) in chain {
            parent.require_name(name.as_os_str(), directory.identity)?;
            if FileIdentity::from_file(&directory.file)? != directory.identity {
                return Err(StorageError::SnapshotChanged);
            }
            parent = directory;
        }
        Ok(())
    }
    fn location(&self) -> Result<bool> {
        self.parent_chain(&self.managed)?;
        self.files
            .root
            .require_name(OsStr::new("trash"), self.files.trash.identity)?;
        if let Some(trash) = &self.trash {
            self.parent_chain(trash)?;
        }
        require_regular(&self.file)?;
        if FileIdentity::from_file(&self.file)? != self.identity {
            return Err(StorageError::SnapshotChanged);
        }
        let m = &self.managed.last().ok_or(StorageError::PathEscape)?.1;
        let observed_trash = match &self.trash {
            Some(chain) => named(
                &chain.last().ok_or(StorageError::PathEscape)?.1,
                &self.trash_name,
            )?,
            None => None,
        };
        match (named(m, &self.managed_name)?, observed_trash) {
            (Some(identity), None) if identity == self.identity => Ok(false),
            (None, Some(identity)) if identity == self.identity => Ok(true),
            _ => Err(StorageError::CustodyRecoveryRequired),
        }
    }
    pub(crate) fn move_to_trash(&mut self, guard: &Custody<'_>) -> Result<()> {
        self.files.check(guard)?;
        self.relocate(true)
    }
    pub(crate) fn restore(&mut self) -> Result<()> {
        self.relocate(false)
    }
    fn relocate(&mut self, to_trash: bool) -> Result<()> {
        self.location()?;
        if to_trash && self.trash.is_none() {
            let (chain, name) = parents(self.files, &self.trash_path, "trash", true)?;
            if name != self.trash_name {
                return Err(StorageError::SnapshotChanged);
            }
            self.trash = Some(chain);
        }
        let m = &self.managed.last().ok_or(StorageError::PathEscape)?.1;
        let t = match &self.trash {
            Some(chain) => &chain.last().ok_or(StorageError::PathEscape)?.1,
            None => &self.files.trash,
        };
        if self.location()? != to_trash {
            let (from, name, to, target) = if to_trash {
                (m, &self.managed_name, t, &self.trash_name)
            } else {
                (t, &self.trash_name, m, &self.managed_name)
            };
            native::renameat_with(
                &from.file,
                name.as_os_str(),
                &to.file,
                target.as_os_str(),
                RenameFlags::NOREPLACE,
            )
            .map_err(|e| StorageError::io("move retained removal archive", e.into()))?;
        }
        // An error retains this object; a retry observes these same descriptors.
        self.location()?;
        durable_file_flush(&self.file)?;
        m.flush()?;
        t.flush()
    }
}
