use crate::{
    Result, StorageError,
    database::WorkCancellation,
    platform::unix::{ConfigTemp, ControlAllocation, Directory, EntryName, durable_file_flush},
};
use std::{
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
};

/// Retained target observation, not permission to initialize, publish, clean up
/// or commit a profile. Revalidation is mandatory at each later effect boundary.
/// No directories are created by inspection; cancellation/picker dismissal has
/// no setup effect. A failed writable probe retains its actual private owners.
#[derive(Debug)]
pub struct TargetPreview {
    requested: PathBuf,
    pub(crate) resolved: PathBuf,
    pub(crate) anchor_path: PathBuf,
    pub(crate) anchor: Directory,
    pub(crate) missing: Vec<OsString>,
    pub(crate) existing: Option<Directory>,
    exclusions: Vec<(PathBuf, PathBuf)>,
    probes: Vec<(Directory, ConfigTemp)>,
    allocations: Vec<(Directory, ControlAllocation)>,
}
impl TargetPreview {
    pub(crate) fn protect(
        &mut self,
        roots: &[&Path],
        cancellation: &WorkCancellation,
    ) -> Result<()> {
        for root in roots {
            self.exclusions
                .push((root.to_path_buf(), resolve(root, false)?.0));
        }
        self.revalidate(cancellation)
    }
    pub fn inspect(
        requested: &Path,
        excluded_roots: &[&Path],
        cancellation: &WorkCancellation,
    ) -> Result<Self> {
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let (resolved, ancestor, missing) = resolve(requested, true)?;
        let exclusions = excluded_roots
            .iter()
            .map(|path| resolve(path, false).map(|p| (path.to_path_buf(), p.0)))
            .collect::<Result<Vec<_>>>()?;
        for (_, path) in &exclusions {
            if physical_overlap(path, &resolved)? {
                return Err(StorageError::SettingsDestination);
            }
        }
        let (anchor_path, missing, existing) = if missing.is_empty() {
            let parent = resolved
                .parent()
                .ok_or(StorageError::SettingsDestination)?
                .to_path_buf();
            let directory = Directory::open(&parent)?;
            let name = EntryName::parse(
                resolved
                    .file_name()
                    .ok_or(StorageError::SettingsDestination)?,
            )?;
            let child = directory.child(name.as_os_str())?;
            (parent, vec![name.as_os_str().to_owned()], Some(child))
        } else {
            (ancestor, missing, None)
        };
        let result = Self {
            requested: requested.to_path_buf(),
            resolved,
            anchor: Directory::open(&anchor_path)?,
            anchor_path,
            missing,
            existing,
            exclusions,
            probes: Vec::new(),
            allocations: Vec::new(),
        };
        result.revalidate(cancellation)?;
        Ok(result)
    }
    pub fn requested(&self) -> &Path {
        &self.requested
    }
    pub fn resolved(&self) -> &Path {
        &self.resolved
    }
    pub fn revalidate(&self, cancellation: &WorkCancellation) -> Result<()> {
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        if !self.probes.is_empty() || !self.allocations.is_empty() {
            return Err(StorageError::SettingsDestination);
        }
        self.anchor.matches_metadata(
            &fs::symlink_metadata(&self.anchor_path)
                .map_err(|e| StorageError::io("revalidate target ancestor", e))?,
        )?;
        if fs::canonicalize(&self.anchor_path)
            .map_err(|e| StorageError::io("resolve retained target ancestor", e))?
            != self.anchor_path
        {
            return Err(StorageError::SnapshotChanged);
        }
        // Re-resolve exclusions too: a configuration/source alias introduced
        // after preview cannot become a destination at commitment.
        let (now, _, _) = resolve(&self.requested, true)?;
        if now != self.resolved {
            return Err(StorageError::SettingsDestination);
        }
        for (requested, resolved) in &self.exclusions {
            let actual = resolve(requested, false)?.0;
            if actual != *resolved || physical_overlap(&actual, &now)? {
                return Err(StorageError::SettingsDestination);
            }
        }
        if let Some(directory) = &self.existing {
            directory.matches_metadata(
                &fs::symlink_metadata(&self.resolved)
                    .map_err(|e| StorageError::io("revalidate empty target", e))?,
            )?;
            let name = self
                .missing
                .last()
                .ok_or(StorageError::SettingsDestination)?;
            let actual = self.anchor.child(name)?;
            if actual.identity() != directory.identity() {
                return Err(StorageError::SnapshotChanged);
            }
            directory.entries(0, cancellation)?;
        } else {
            let first = self
                .missing
                .first()
                .ok_or(StorageError::SettingsDestination)?;
            match fs::symlink_metadata(self.anchor_path.join(first)) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Ok(_) => return Err(StorageError::SnapshotChanged),
                Err(error) => {
                    return Err(StorageError::io("revalidate absent target ancestor", error));
                }
            }
        }
        Ok(())
    }
    /// Exercise actual create-new, file flush, directory flush and owned-temp
    /// retirement at the retained parent and (when present) selected directory.
    /// This establishes capabilities at observation time, not future free space.
    pub fn check_writable(&mut self, cancellation: &WorkCancellation) -> Result<()> {
        self.revalidate(cancellation)?;
        self.check_missing_aliases(cancellation)?;
        let mut directories = vec![self.anchor.duplicate()?];
        if let Some(existing) = &self.existing {
            directories.push(existing.duplicate()?);
        }
        for directory in directories {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let temporary = match directory.create_control_temp(".librairii-target-probe-") {
                Ok(temporary) => temporary,
                Err(failure) => {
                    let failure = *failure;
                    if failure.allocation.file.is_some() {
                        self.allocations.push((directory, failure.allocation));
                    }
                    return Err(failure.cause);
                }
            };
            self.probes.push((directory, temporary));
            let (directory, temporary) = self
                .probes
                .last()
                .ok_or(StorageError::SettingsDestination)?;
            durable_file_flush(&temporary.file)?;
            directory.flush()?;
            if temporary
                .file
                .metadata()
                .map_err(|e| StorageError::io("inspect writable probe", e))?
                .len()
                != 0
            {
                return Err(StorageError::SnapshotChanged);
            }
            directory.remove_config_temp(temporary)?;
            directory.flush()?;
            self.probes.pop();
        }
        self.revalidate(cancellation)
    }
    fn check_missing_aliases(&mut self, cancellation: &WorkCancellation) -> Result<()> {
        use std::os::unix::ffi::OsStringExt;
        let (_, _, target_tail) = resolve(&self.requested, true)?;
        let Some(target_name) = target_tail.first() else {
            return Ok(());
        };
        for (excluded, _) in self.exclusions.clone() {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let (_, parent, missing) = resolve(&excluded, false)?;
            // A missing immediate protected root (the old automatic default)
            // can alias the selected first component on a folding filesystem.
            // Probe the actual parent, using private names, before mkdir effects.
            if missing.len() != 1 || Directory::open(&parent)?.identity() != self.anchor.identity()
            {
                continue;
            }
            let Some(protected) = missing.first() else {
                continue;
            };
            if protected == target_name {
                return Err(StorageError::SettingsDestination);
            }
            let protected = protected
                .to_str()
                .ok_or(StorageError::SettingsDestination)?;
            let prefix = format!(".librairii-alias-{protected}-");
            let directory = self.anchor.duplicate()?;
            let temporary = match directory.create_control_temp(&prefix) {
                Ok(temporary) => temporary,
                Err(failure) => {
                    let failure = *failure;
                    if failure.allocation.file.is_some() {
                        self.allocations.push((directory, failure.allocation));
                    }
                    return Err(failure.cause);
                }
            };
            let mut alternate = OsString::from(".librairii-alias-");
            alternate.push(target_name);
            alternate.push("-");
            alternate.push(OsString::from_vec(
                temporary.name.as_encoded_bytes()[prefix.len()..].to_vec(),
            ));
            self.probes.push((directory, temporary));
            let (directory, temporary) = self
                .probes
                .last()
                .ok_or(StorageError::SettingsDestination)?;
            let alias = match directory.read_regular(&alternate)? {
                Some(file) => crate::settings::manifest::same_file(&file, &temporary.file)?,
                None => false,
            };
            if temporary
                .file
                .metadata()
                .map_err(|e| StorageError::io("observe private alias probe", e))?
                .len()
                != 0
            {
                return Err(StorageError::SnapshotChanged);
            }
            directory.remove_config_temp(temporary)?;
            directory.flush()?;
            self.probes.pop();
            if alias {
                return Err(StorageError::SettingsDestination);
            }
        }
        Ok(())
    }
}

/// realpath may preserve alternate case/decomposition spellings on a folding
/// volume. Compare actual ancestor identities as well as lexical containment.
/// For nonexistent paths, effects must recheck after exclusive creation too.
pub(crate) fn physical_overlap(a: &Path, b: &Path) -> Result<bool> {
    Ok(physical_contains(a, b)? || physical_contains(b, a)?)
}
pub(crate) fn physical_contains(root: &Path, descendant: &Path) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    if descendant.starts_with(root) {
        return Ok(true);
    }
    let root_metadata = match fs::metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(StorageError::io("inspect excluded root identity", error)),
    };
    let mut ancestor = resolve(descendant, false)?.1;
    loop {
        let metadata = fs::metadata(&ancestor)
            .map_err(|e| StorageError::io("inspect target ancestry identity", e))?;
        if metadata.dev() == root_metadata.dev() && metadata.ino() == root_metadata.ino() {
            return Ok(true);
        }
        if !ancestor.pop() {
            break;
        }
    }
    Ok(false)
}

/// Resolve through existing ancestors while retaining a missing tail. Existing
/// final aliases and special files are refused; ancestor aliases are resolved
/// before source/configuration overlap is checked. No lossy path conversion.
pub(crate) fn resolve(
    path: &Path,
    reject_final_alias: bool,
) -> Result<(PathBuf, PathBuf, Vec<OsString>)> {
    if !path.is_absolute()
        || path.components().any(|c| matches!(c, Component::ParentDir))
        || path.as_os_str().as_encoded_bytes().contains(&0)
    {
        return Err(StorageError::SettingsDestination);
    }
    if reject_final_alias {
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                return Err(StorageError::SettingsDestination);
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(StorageError::io("inspect selected target", error)),
        }
    }
    let mut cursor = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match fs::canonicalize(&cursor) {
            Ok(ancestor) => {
                if !fs::metadata(&ancestor)
                    .map_err(|e| StorageError::io("inspect target ancestor", e))?
                    .is_dir()
                {
                    return Err(StorageError::SettingsDestination);
                }
                missing.reverse();
                let mut resolved = ancestor.clone();
                for name in &missing {
                    resolved.push(name);
                }
                return Ok((resolved, ancestor, missing));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::symlink_metadata(&cursor) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                    _ => return Err(StorageError::SettingsDestination),
                }
                missing.push(
                    cursor
                        .file_name()
                        .ok_or(StorageError::SettingsDestination)?
                        .to_owned(),
                );
                if !cursor.pop() {
                    return Err(StorageError::SettingsDestination);
                }
            }
            Err(error) => return Err(StorageError::io("resolve selected target", error)),
        }
    }
}
