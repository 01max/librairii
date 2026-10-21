//! Explicit isolated initialization; no home, profile or environment discovery.
use crate::{LibraryIdentity, Result, StorageError};
use librairii_core::LibraryId;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const DATA_ROOT_OVERRIDE: &str = "LIBRAIRII_DATA_ROOT";
const DIRECTORIES: [&str; 6] = ["db", "archives", "catalog", "staging", "trash", "logs"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryLayout {
    root: PathBuf,
}

impl LibraryLayout {
    /// Describes an existing root without initialization, identity repair or profile access.
    pub fn probe(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        let metadata =
            fs::symlink_metadata(root).map_err(|e| StorageError::io("inspect library root", e))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(StorageError::PathEscape);
        }
        let root =
            fs::canonicalize(root).map_err(|e| StorageError::io("resolve library root", e))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn database(&self) -> PathBuf {
        self.root.join("db").join("librairii.sqlite3")
    }
    pub fn archives(&self) -> PathBuf {
        self.root.join("archives")
    }
    pub fn catalog(&self) -> PathBuf {
        self.root.join("catalog")
    }
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }
    pub fn trash(&self) -> PathBuf {
        self.root.join("trash")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn directories(&self) -> Vec<PathBuf> {
        std::iter::once(self.root.clone())
            .chain(DIRECTORIES.iter().map(|name| self.root.join(name)))
            .collect()
    }

    /// Resolve existing ancestors without following a link out of managed custody.
    /// This is path validation, not publication/cleanup authority or a race-proof OS handle.
    pub fn resolve(&self, relative: &ManagedPath) -> Result<PathBuf> {
        let metadata = fs::symlink_metadata(&self.root)
            .map_err(|e| StorageError::io("recheck library root", e))?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || fs::canonicalize(&self.root)
                .map_err(|e| StorageError::io("recheck library root identity", e))?
                != self.root
        {
            return Err(StorageError::PathEscape);
        }
        let mut candidate = self.root.clone();
        for part in relative.parts()? {
            let mut native = Path::new(&part).components();
            if !matches!(native.next(), Some(Component::Normal(_))) || native.next().is_some() {
                return Err(StorageError::InvalidManagedPath);
            }
            candidate.push(part);
            match fs::symlink_metadata(&candidate) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(StorageError::PathEscape);
                }
                Ok(_) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(StorageError::io("inspect managed path", error)),
            }
        }
        Ok(candidate)
    }
}

/// Go-compatible relative managed TEXT. Original spelling is retained verbatim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedPath(String);

impl ManagedPath {
    pub fn parse(raw: impl Into<String>) -> Result<Self> {
        let value = Self(raw.into());
        value.parts()?;
        Ok(value)
    }
    pub fn as_stored(&self) -> &str {
        &self.0
    }
    fn parts(&self) -> Result<Vec<OsString>> {
        if self.0.is_empty() || self.0.starts_with('/') || self.0.contains(['\\', '\0']) {
            return Err(StorageError::InvalidManagedPath);
        }
        let mut parts = Vec::new();
        for part in self.0.split('/') {
            match part {
                "" | "." => (),
                ".." => {
                    if parts.pop().is_none() {
                        return Err(StorageError::InvalidManagedPath);
                    }
                }
                _ => parts.push(OsString::from(part)),
            }
        }
        if parts.is_empty() {
            return Err(StorageError::InvalidManagedPath);
        }
        Ok(parts)
    }
}

/// Authority for this fresh explicit development root, never a production profile.
/// Existing libraries must use the separately verified adoption/lease route.
#[derive(Debug)]
pub struct IsolatedLibrary {
    layout: LibraryLayout,
    identity: LibraryIdentity,
    runtime_active: AtomicBool,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) custody_active: AtomicBool,
    pub(crate) custody_fenced: Arc<AtomicBool>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    managed_metadata: Vec<fs::Metadata>,
    lease: crate::platform::lock::LibraryLease,
}

impl IsolatedLibrary {
    /// The caller supplies the override value and a fresh typed identity. Neither
    /// global environment nor production preferences are read or modified here.
    pub fn initialize_from_override(value: Option<&OsStr>, identity: LibraryId) -> Result<Self> {
        let root = Path::new(value.ok_or(StorageError::MissingOverride)?);
        if !root.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        match fs::symlink_metadata(root) {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(StorageError::RootNotFresh);
                }
                if fs::read_dir(root)
                    .map_err(|e| StorageError::io("inspect fresh root", e))?
                    .next()
                    .is_some()
                {
                    return Err(StorageError::RootNotFresh);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = root.parent().ok_or(StorageError::RootNotAbsolute)?;
                let mut parents = fs::DirBuilder::new();
                parents.recursive(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    parents.mode(0o700);
                }
                parents
                    .create(parent)
                    .map_err(|e| StorageError::io("create isolated root parents", e))?;
                create_private_dir(root)?;
            }
            Err(error) => return Err(StorageError::io("inspect isolated override", error)),
        }
        let layout = LibraryLayout::probe(root)?;
        let lease = crate::platform::lock::LibraryLease::create_fresh(layout.root())?;
        let identity = LibraryIdentity::create_new(layout.root(), identity)?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let mut managed_metadata = Vec::with_capacity(DIRECTORIES.len());
        for name in DIRECTORIES {
            create_private_dir(&layout.root.join(name))?;
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            managed_metadata.push(
                fs::symlink_metadata(layout.root.join(name)).map_err(|e| {
                    StorageError::io("retain initial managed directory identity", e)
                })?,
            );
        }
        Ok(Self {
            layout,
            identity,
            runtime_active: AtomicBool::new(false),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            custody_active: AtomicBool::new(false),
            custody_fenced: Arc::new(AtomicBool::new(false)),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            managed_metadata,
            lease,
        })
    }
    pub fn layout(&self) -> &LibraryLayout {
        &self.layout
    }
    pub fn identity(&self) -> &LibraryIdentity {
        &self.identity
    }
    pub const fn relocation_allowed(&self) -> bool {
        false
    }
    /// Cooperative OS ownership retained beyond runtime drain. It does not
    /// prove exclusion of an unmodified legacy executable or authorize adoption.
    pub fn lease(&self) -> &crate::platform::lock::LibraryLease {
        &self.lease
    }
    pub fn revalidate_ownership(&self) -> Result<()> {
        self.revalidate_lease_identity()?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        for (name, expected) in DIRECTORIES.iter().zip(&self.managed_metadata) {
            self.require_managed_identity(name, expected)?;
        }
        Ok(())
    }
    fn revalidate_lease_identity(&self) -> Result<()> {
        self.lease.revalidate()?;
        if LibraryIdentity::probe(self.layout.root())?.as_ref() != Some(&self.identity) {
            return Err(StorageError::IdentityMismatch);
        }
        Ok(())
    }
    /// Required DB-only reporting/readback remains possible when an archive or
    /// staging name is uncertain. Root/identity/OS lease and the ORIGINAL db
    /// directory still must validate; this cannot activate replacement roots.
    pub(crate) fn revalidate_database_ownership(&self) -> Result<()> {
        self.revalidate_lease_identity()?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        self.require_managed_identity("db", &self.managed_metadata[0])?;
        Ok(())
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn require_managed_identity(&self, name: &str, expected: &fs::Metadata) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let actual = fs::symlink_metadata(self.layout.root.join(name))
            .map_err(|e| StorageError::io("revalidate original managed directory", e))?;
        if !actual.is_dir()
            || actual.file_type().is_symlink()
            || actual.dev() != expected.dev()
            || actual.ino() != expected.ino()
        {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(())
    }
    pub(crate) fn claim_runtime(&self) -> Result<()> {
        self.revalidate_ownership()?;
        if self.custody_fenced.load(Ordering::Acquire) {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        self.runtime_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| StorageError::RuntimeInUse)
    }
    pub(crate) fn release_runtime(&self) {
        self.runtime_active.store(false, Ordering::Release);
    }
}

fn create_private_dir(path: &Path) -> Result<()> {
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let mut builder = builder;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|e| StorageError::io("create private library directory", e))
}
