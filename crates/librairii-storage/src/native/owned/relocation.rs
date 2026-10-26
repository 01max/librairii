//! Closed-owner relocation. Profile publication is the single commit point.
//! There is no recursive deletion, destructive Drop, or path-only cleanup API.
use super::super::state::MoveProof;
use super::*;
use crate::{
    platform::{
        hash_stream,
        unix::{FileIdentity, durable_file_flush},
    },
    settings::{RelocationManifest, TargetPreview, manifest::MoveEntry},
};
use librairii_core::LibraryId;
use serde::{Deserialize, Serialize};
use std::io::{Seek, Write};

#[derive(Debug)]
pub(crate) struct VerifiedMoveTarget {
    pub(super) root: String,
    pub(super) raw_id: String,
}
impl VerifiedMoveTarget {
    pub(crate) fn root(&self) -> &str {
        &self.root
    }
    pub(crate) fn raw_id(&self) -> &str {
        &self.raw_id
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    format: String,
    version: u64,
    id: String,
    raw_id: String,
    profile_sha256: String,
    source: String,
    target: String,
    workspace: String,
    #[serde(default)]
    selected_workspace: bool,
    source_directory: FileIdentity,
    workspace_directory: FileIdentity,
    source_state_sha256: String,
    manifest_sha256: String,
    manifest: RelocationManifest,
}
impl Intent {
    fn bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| StorageError::RecoveryBindingRequired)?;
        if bytes.len() > crate::profile::relocation::MAX_JOURNAL {
            return Err(StorageError::HashSizeLimit);
        }
        Ok(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > crate::profile::relocation::MAX_JOURNAL {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(Self::decode_journal(bytes)?.0)
    }
    fn decode_journal(bytes: &[u8]) -> Result<(Self, bool)> {
        if bytes.len() > crate::profile::relocation::MAX_JOURNAL {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let end = bytes
            .iter()
            .position(|b| *b == b'\n')
            .unwrap_or(bytes.len());
        let intent: Self = serde_json::from_slice(&bytes[..end])
            .map_err(|_| StorageError::RecoveryBindingRequired)?;
        let canonical = intent.bytes()?;
        if canonical != bytes[..end] {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let footer = format!("\nretired:{}\n", digest(&canonical));
        let suffix = &bytes[end..];
        if !footer.as_bytes().starts_with(suffix) {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok((intent, !suffix.is_empty() && suffix == footer.as_bytes()))
    }
    fn validate(&self) -> Result<()> {
        self.id
            .parse::<LibraryId>()
            .map_err(|_| StorageError::RecoveryBindingRequired)?;
        LibraryIdentity::from_bytes(self.raw_id.as_bytes().to_vec())?;
        if self.format != "librairii.native-relocation"
            || self.version != 1
            || digest(&self.manifest.bytes()?) != self.manifest_sha256
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        for value in [
            &self.profile_sha256,
            &self.source_state_sha256,
            &self.manifest_sha256,
        ] {
            value
                .parse::<librairii_core::Checksum>()
                .map_err(|_| StorageError::RecoveryBindingRequired)?;
        }
        for path in [&self.source, &self.target, &self.workspace] {
            let p = Path::new(path);
            if !p.is_absolute()
                || p.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir | std::path::Component::CurDir
                    )
                })
            {
                return Err(StorageError::RecoveryBindingRequired);
            }
        }
        let target = Path::new(&self.target);
        let workspace = Path::new(&self.workspace);
        if (if self.selected_workspace {
            target != workspace
        } else {
            target.parent() != workspace.parent()
                || workspace.file_name()
                    != Some(OsStr::new(&format!(".librairii-native-move-{}", self.id)))
        }) || crate::settings::target::physical_overlap(Path::new(&self.source), target)?
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(())
    }
    fn proof(&self) -> MoveProof {
        MoveProof {
            id: self.id.clone(),
            manifest_sha256: self.manifest_sha256.clone(),
            source_state_sha256: self.source_state_sha256.clone(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CleanupReport {
    pub pending: bool,
    pub preserved_files: usize,
}
#[derive(Debug)]
pub struct RelocatedLibrary<'profile> {
    pub library: NativeLibrary<'profile>,
    pub cleanup: CleanupReport,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Point {
    Journal,
    File,
    Published,
    BeforeCommit,
    Committed,
    Retired,
}
#[derive(Debug)]
struct Workspace {
    parent: Directory,
    parent_path: PathBuf,
    name: EntryName,
    target_name: EntryName,
    root: Directory,
    record: Option<Record>,
    lease: Option<LibraryLease>,
    document: Document,
    allocation: Option<ControlAllocation>,
    lease_attempted: bool,
    published: bool,
}
#[derive(Debug)]
struct Attempt<'profile> {
    publication_uncertain: bool,
    source: Option<ClosedNativeLibrary<'profile>>,
    destination: Option<NativeLibrary<'profile>>,
    target: TargetPreview,
    locations: ProfileLocations,
    id: LibraryId,
    workspace: Option<Workspace>,
    intent: Option<Intent>,
    journal: bool,
    parents: Vec<(PathBuf, Directory)>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitStatus {
    BeforeCommit,
    Committed,
    Uncertain,
}
#[derive(Debug)]
pub struct RelocationFailure<'profile> {
    cause: StorageError,
    attempt: Attempt<'profile>,
}
impl std::fmt::Display for RelocationFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "library move failed: {}", self.cause)
    }
}
impl std::error::Error for RelocationFailure<'_> {}
impl<'profile> RelocationFailure<'profile> {
    pub fn storage_error(&self) -> &StorageError {
        &self.cause
    }
    pub fn retained_destination(&self) -> Option<&Path> {
        self.attempt.intent.as_ref().map(|intent| {
            if self
                .attempt
                .workspace
                .as_ref()
                .is_some_and(|workspace| workspace.published)
            {
                Path::new(&intent.target)
            } else {
                Path::new(&intent.workspace)
            }
        })
    }
    pub fn commit_status(&self) -> CommitStatus {
        if self.attempt.destination.is_some() {
            CommitStatus::Committed
        } else if self.attempt.publication_uncertain {
            CommitStatus::Uncertain
        } else {
            CommitStatus::BeforeCommit
        }
    }
    pub fn retry(
        self: Box<Self>,
        cancel: &WorkCancellation,
    ) -> std::result::Result<RelocatedLibrary<'profile>, Box<Self>> {
        self.attempt.complete(cancel, |_| Ok(()))
    }
    pub fn resume_source(
        mut self: Box<Self>,
    ) -> std::result::Result<(NativeLibrary<'profile>, CleanupReport), Box<Self>> {
        if self.commit_status() != CommitStatus::BeforeCommit {
            return Err(self);
        }
        match self.attempt.abort() {
            Ok(report) => match self.attempt.source.take() {
                Some(source) => Ok((source.resume(), report)),
                None => Err(self),
            },
            Err(cause) => {
                self.cause = cause;
                Err(self)
            }
        }
    }
}
impl<'profile> ClosedNativeLibrary<'profile> {
    pub fn relocate(
        self,
        target: TargetPreview,
        locations: &ProfileLocations,
        id: LibraryId,
        cancel: &WorkCancellation,
    ) -> std::result::Result<RelocatedLibrary<'profile>, Box<RelocationFailure<'profile>>> {
        Attempt {
            publication_uncertain: false,
            source: Some(self),
            destination: None,
            target,
            locations: locations.clone(),
            id,
            workspace: None,
            intent: None,
            journal: false,
            parents: Vec::new(),
        }
        .complete(cancel, |_| Ok(()))
    }
}
impl<'profile> Attempt<'profile> {
    fn complete(
        mut self,
        cancel: &WorkCancellation,
        mut checkpoint: impl FnMut(Point) -> Result<()>,
    ) -> std::result::Result<RelocatedLibrary<'profile>, Box<RelocationFailure<'profile>>> {
        match self.run(cancel, &mut checkpoint) {
            Ok(outcome) => Ok(outcome),
            Err(cause) => Err(Box::new(RelocationFailure {
                cause,
                attempt: self,
            })),
        }
    }
    fn run(
        &mut self,
        cancel: &WorkCancellation,
        checkpoint: &mut impl FnMut(Point) -> Result<()>,
    ) -> Result<RelocatedLibrary<'profile>> {
        if self.publication_uncertain {
            return Err(StorageError::ProfileRestartRequired);
        }
        if self.destination.is_some() {
            return self.finish(checkpoint);
        }
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let source = self
            .source
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        source.revalidate()?;
        let store = source.library.binding()?.store();
        if self.workspace.is_none() {
            let record = source
                .library
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            let source_document = record.document.clone();
            let source_state_sha256 = digest(&record.bytes);
            drop(record);
            let policy = source_document
                .root_policy
                .clone()
                .unwrap_or(RootPolicy::for_source(
                    Path::new(&source_document.source_root),
                    &self.locations,
                )?);
            let mut excluded = vec![source.layout().root(), self.locations.native.as_path()];
            if let Some(legacy) = &self.locations.legacy {
                excluded.push(legacy);
            }
            for root in &policy.legacy_roots {
                excluded.push(Path::new(root));
            }
            if self.parents.is_empty() {
                self.target.protect(&excluded, cancel)?;
                self.target.check_writable(cancel)?;
            }
            require_explicit_paths(
                self.target.resolved(),
                Path::new(&source_document.source_root),
                &policy,
                store,
                &self.locations,
            )?;
            let manifest = source.relocation_manifest(cancel)?;
            self.target.check_capacity(
                manifest
                    .summary()?
                    .bytes
                    .checked_add(4 * 1024 * 1024)
                    .ok_or(StorageError::HashSizeLimit)?,
            )?;
            let mut parent = self.target.anchor.duplicate()?;
            let mut parent_path = self.target.anchor_path.clone();
            for (index, name) in self
                .target
                .missing
                .iter()
                .take(self.target.missing.len().saturating_sub(1))
                .enumerate()
            {
                parent_path.push(name);
                let child = if let Some((path, owned)) = self.parents.get(index) {
                    if path != &parent_path {
                        return Err(StorageError::SnapshotChanged);
                    }
                    let child = parent.child(name)?;
                    if child.identity() != owned.identity() {
                        return Err(StorageError::SnapshotChanged);
                    }
                    child
                } else {
                    let child = parent.create_child(name)?;
                    self.parents.push((parent_path.clone(), child.duplicate()?));
                    child
                };
                parent.flush()?;
                parent = child;
            }
            let target_name = EntryName::parse(
                self.target
                    .missing
                    .last()
                    .ok_or(StorageError::SettingsDestination)?,
            )?;
            let selected_workspace = if let Some(existing) = &self.target.existing {
                existing.metadata()?.dev() != self.target.anchor.metadata()?.dev()
            } else {
                false
            };
            let name = if selected_workspace {
                target_name.clone()
            } else {
                EntryName::parse(format!(".librairii-native-move-{}", self.id))?
            };
            let path = parent_path.join(name.as_os_str());
            let profile_sha256 = store.marker_digest()?;
            let source_path = exact_path(source.layout().root())?;
            let target_path = exact_path(self.target.resolved())?;
            let workspace_path = exact_path(&path)?;
            let manifest_sha256 = digest(&manifest.bytes()?);
            let root = if selected_workspace {
                self.target
                    .existing
                    .as_ref()
                    .ok_or(StorageError::CompatibilityInvariant)?
                    .duplicate()?
            } else {
                parent.create_child(name.as_os_str())?
            };
            let intent = Intent {
                format: "librairii.native-relocation".into(),
                version: 1,
                id: self.id.to_string(),
                raw_id: source.identity().spelling().into(),
                profile_sha256,
                source: source_path,
                target: target_path,
                workspace: workspace_path,
                selected_workspace,
                source_directory: source.library.root.identity(),
                workspace_directory: root.identity(),
                source_state_sha256,
                manifest_sha256,
                manifest,
            };
            let mut document = source_document;
            document.version = 2;
            document.library_root = intent.target.clone();
            let mut policy = policy;
            policy.relocation = Some(intent.proof());
            document.root_policy = Some(policy);
            self.intent = Some(intent);
            self.workspace = Some(Workspace {
                parent,
                parent_path,
                name,
                target_name,
                root,
                record: None,
                lease: None,
                document,
                allocation: None,
                lease_attempted: false,
                published: false,
            });
        }
        let intent = self
            .intent
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let workspace = self
            .workspace
            .as_mut()
            .ok_or(StorageError::CompatibilityInvariant)?;
        workspace.parent.matches_metadata(
            &fs::symlink_metadata(&workspace.parent_path)
                .map_err(|e| StorageError::io("revalidate move parent", e))?,
        )?;
        for (path, directory) in &self.parents {
            directory.matches_metadata(
                &fs::symlink_metadata(path)
                    .map_err(|e| StorageError::io("revalidate created move parent", e))?,
            )?;
        }
        workspace.parent.flush()?;
        if workspace.lease.is_none() {
            let path = if workspace.published {
                Path::new(&intent.target)
            } else {
                Path::new(&intent.workspace)
            };
            workspace.lease = Some(
                if workspace.lease_attempted
                    && workspace
                        .root
                        .read_regular(OsStr::new(crate::platform::lock::LIBRARY_LOCK_FILE))?
                        .is_some()
                {
                    LibraryLease::acquire_existing_in(path, &workspace.root)?
                } else {
                    workspace.lease_attempted = true;
                    LibraryLease::create_fresh_in(path, &workspace.root)?
                },
            );
        }
        if workspace.record.is_none() {
            if let Some(allocation) = &workspace.allocation {
                workspace.record = Some(Record::from_owned_allocation(
                    &workspace.root,
                    workspace.document.clone(),
                    allocation,
                )?);
            } else {
                workspace.record = Some(
                    Record::create(&workspace.root, workspace.document.clone()).map_err(
                        |failure| {
                            workspace.allocation = failure.allocation;
                            failure.cause
                        },
                    )?,
                );
            }
        }
        let record = workspace
            .record
            .as_mut()
            .ok_or(StorageError::CompatibilityInvariant)?;
        if record.uncertain {
            record.initialize(&workspace.root)?;
        }
        // A no-clobber rename may have succeeded before its flush ACK failed.
        if !workspace.published
            && !intent.selected_workspace
            && workspace
                .parent
                .child(workspace.target_name.as_os_str())
                .is_ok_and(|actual| actual.identity() == workspace.root.identity())
        {
            workspace.published = true;
        }
        if workspace.published {
            workspace
                .lease
                .as_mut()
                .ok_or(StorageError::CompatibilityInvariant)?
                .retarget_published(Path::new(&intent.target), &workspace.root)?;
        }
        if !self.journal {
            store.create_move_journal(&intent.bytes()?)?;
            self.journal = true;
            checkpoint(Point::Journal)?;
        }
        workspace
            .lease
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .revalidate()?;
        workspace
            .record
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .revalidate(&workspace.root)?;
        if !workspace.published {
            for entry in &intent.manifest.entries {
                if control(entry) {
                    continue;
                }
                copy_entry(&source.library.root, &workspace.root, entry, cancel)?;
                if !entry.directory {
                    checkpoint(Point::File)?;
                }
            }
            verify_copy(&workspace.root, &intent.manifest, cancel)?;
            require_no_sidecars(&workspace.root.child(OsStr::new("db"))?)?;
            let scratch = tempfile::tempdir()
                .map_err(|e| StorageError::io("create move integrity probe", e))?;
            crate::backup::probe_owned(
                &Path::new(&intent.workspace).join("db/librairii.sqlite3"),
                scratch.path(),
            )?;
            if source.relocation_manifest(cancel)? != intent.manifest {
                return Err(StorageError::SnapshotChanged);
            }
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            if intent.selected_workspace {
                // An empty mounted volume root cannot be renamed/unmounted.
                // Its retained empty directory becomes the exclusive workspace;
                // every child publication was create-new/no-clobber. The
                // independent profile is still the sole authority commit.
                workspace.published = true;
            } else {
                // The empty target was retained at preview. Never replace a file,
                // populated directory, or a newly substituted target.
                if let Some(existing) = &self.target.existing {
                    workspace
                        .parent
                        .remove_empty_child(workspace.target_name.as_os_str(), existing)?;
                    self.target.existing = None;
                } else if workspace
                    .parent
                    .child(workspace.target_name.as_os_str())
                    .is_ok()
                {
                    return Err(StorageError::SnapshotChanged);
                }
                workspace.parent.publish_directory(
                    workspace.name.as_os_str(),
                    &workspace.root,
                    workspace.target_name.as_os_str(),
                )?;
                workspace.published = true;
                workspace
                    .lease
                    .as_mut()
                    .ok_or(StorageError::CompatibilityInvariant)?
                    .retarget_published(Path::new(&intent.target), &workspace.root)?;
            }
            checkpoint(Point::Published)?;
        }
        workspace
            .record
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .revalidate(&workspace.root)?;
        workspace
            .lease
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .revalidate()?;
        verify_copy(&workspace.root, &intent.manifest, cancel)?;
        if source.relocation_manifest(cancel)? != intent.manifest {
            return Err(StorageError::SnapshotChanged);
        }
        require_explicit_paths(
            Path::new(&intent.target),
            Path::new(
                &workspace
                    .record
                    .as_ref()
                    .ok_or(StorageError::CompatibilityInvariant)?
                    .document
                    .source_root,
            ),
            workspace
                .record
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?
                .document
                .root_policy
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?,
            store,
            &self.locations,
        )?;
        let layout = LibraryLayout::probe(Path::new(&intent.target))?;
        let database = workspace.root.child(OsStr::new("db"))?;
        crate::database::ready_closed_move(
            &Path::new(&intent.target).join("db/librairii.sqlite3"),
        )?;
        require_no_sidecars(&database)?;
        workspace
            .lease
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .revalidate()?;
        verify_copy(&workspace.root, &intent.manifest, cancel)?;
        let marker = workspace.root.open_regular(
            OsStr::new(crate::identity::IDENTITY_FILE),
            rustix::fs::OFlags::RDONLY,
        )?;
        let mut directories = Vec::new();
        for name in ["db", "archives", "catalog", "staging", "trash", "logs"] {
            directories.push((name, workspace.root.child(OsStr::new(name))?));
        }
        checkpoint(Point::BeforeCommit)?;
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let proof = VerifiedMoveTarget {
            root: intent.target.clone(),
            raw_id: intent.raw_id.clone(),
        };
        let source = self
            .source
            .as_mut()
            .ok_or(StorageError::CompatibilityInvariant)?;
        self.publication_uncertain = true;
        let binding = source
            .library
            .binding
            .as_mut()
            .ok_or(StorageError::RuntimeFenced)?
            .retarget(&proof)?;
        self.publication_uncertain = false;
        let workspace = self
            .workspace
            .take()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let owner = RootOwner::Existing(Box::new(ExistingRoot {
            parent: workspace.parent,
            name: workspace.target_name,
            directories,
            lease: workspace
                .lease
                .ok_or(StorageError::CompatibilityInvariant)?,
        }));
        let native = NativeLibrary::new(
            NativeDirectories {
                layout,
                root: workspace.root,
                database,
            },
            marker,
            source.identity().clone(),
            workspace
                .record
                .ok_or(StorageError::CompatibilityInvariant)?,
            owner,
            binding,
        );
        self.destination = Some(native);
        checkpoint(Point::Committed)?;
        self.finish(checkpoint)
    }
    fn finish(
        &mut self,
        checkpoint: &mut impl FnMut(Point) -> Result<()>,
    ) -> Result<RelocatedLibrary<'profile>> {
        let source = self
            .source
            .as_mut()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let store = source.library.binding()?.store();
        let intent = self
            .intent
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let cleanup = cleanup_tree(
            &source.library.root,
            &intent.manifest,
            Some((&intent.source_state_sha256, None)),
            false,
            || {
                store.acknowledge_move_retirement(&intent.bytes()?)?;
                checkpoint(Point::Retired)
            },
        )
        .unwrap_or(CleanupReport {
            pending: true,
            preserved_files: 1,
        });
        let mut report = cleanup;
        if !report.pending {
            let path = source.layout().root();
            let parent = Directory::open(path.parent().ok_or(StorageError::RootNotAbsolute)?)?;
            if parent
                .remove_empty_child(
                    path.file_name().ok_or(StorageError::RootNotAbsolute)?,
                    &source.library.root,
                )
                .is_err()
            {
                report.pending = true;
            }
        }
        if !report.pending {
            store.retire_move_journal()?;
        }
        source.library.finalized = true;
        self.source.take();
        Ok(RelocatedLibrary {
            library: self
                .destination
                .take()
                .ok_or(StorageError::CompatibilityInvariant)?,
            cleanup: report,
        })
    }
    fn abort(&mut self) -> Result<CleanupReport> {
        let source = self
            .source
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let store = source.library.binding()?.store();
        source.revalidate()?;
        // Incomplete destination files have no durable per-file receipt.
        // Keep them rather than deriving deletion authority from intent bytes.
        let report = if self.workspace.is_some() {
            CleanupReport {
                pending: true,
                preserved_files: 1,
            }
        } else {
            CleanupReport::default()
        };
        if self.journal {
            store.retire_move_journal()?;
            self.journal = false;
        }
        Ok(report)
    }
}
fn control(entry: &MoveEntry) -> bool {
    entry.path.len() == 1
        && [
            NATIVE_STATE_FILE.as_bytes(),
            crate::platform::lock::LIBRARY_LOCK_FILE.as_bytes(),
        ]
        .contains(&entry.path[0].as_slice())
}
fn parent_for(root: &Directory, entry: &MoveEntry) -> Result<(Directory, std::ffi::OsString)> {
    let mut names = entry.names()?;
    let last = names.pop().ok_or(StorageError::InvalidManagedPath)?;
    let mut parent = root.duplicate()?;
    for name in names {
        parent = parent.child(&name)?;
    }
    Ok((parent, last))
}
fn file_matches(file: &mut File, entry: &MoveEntry, cancel: &WorkCancellation) -> Result<bool> {
    file.rewind()
        .map_err(|e| StorageError::io("rewind move file", e))?;
    let measured = hash_stream(file, entry.bytes, cancel)?;
    Ok(measured.bytes() == entry.bytes
        && Some(measured.checksum().to_string()).as_ref() == entry.sha256.as_ref())
}
fn copy_entry(
    source: &Directory,
    target: &Directory,
    entry: &MoveEntry,
    cancel: &WorkCancellation,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(StorageError::Cancelled);
    }
    let (parent, name) = parent_for(target, entry)?;
    if entry.directory {
        match parent.child(&name) {
            Ok(_) => (),
            Err(StorageError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                parent.create_child(&name)?;
                parent.flush()?;
            }
            Err(e) => return Err(e),
        };
        return Ok(());
    }
    let (source_parent, source_name) = parent_for(source, entry)?;
    let mut input = source_parent.open_regular(&source_name, rustix::fs::OFlags::RDONLY)?;
    if FileIdentity::from_file(&input)? != entry.identity
        || crate::settings::manifest::modified(
            &input
                .metadata()
                .map_err(|e| StorageError::io("observe move source", e))?,
        ) != entry.modified
    {
        return Err(StorageError::SnapshotChanged);
    }
    if let Some(mut existing) = parent.read_regular(&name)? {
        if file_matches(&mut existing, entry, cancel)? {
            return Ok(());
        }
        return Err(StorageError::SnapshotChanged);
    }
    let mut output = parent.open_regular(
        &name,
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CREATE | rustix::fs::OFlags::EXCL,
    )?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let size = input
            .read(&mut buffer)
            .map_err(|e| StorageError::io("read relocation source", e))?;
        if size == 0 {
            break;
        }
        written = written
            .checked_add(size as u64)
            .ok_or(StorageError::HashSizeLimit)?;
        if written > entry.bytes {
            return Err(StorageError::SnapshotChanged);
        }
        output
            .write_all(&buffer[..size])
            .map_err(|e| StorageError::io("write relocation copy", e))?;
    }
    durable_file_flush(&output)?;
    parent.flush()?;
    if !file_matches(&mut output, entry, cancel)?
        || FileIdentity::from_file(
            &source_parent.open_regular(&source_name, rustix::fs::OFlags::RDONLY)?,
        )? != entry.identity
        || crate::settings::manifest::modified(
            &input
                .metadata()
                .map_err(|e| StorageError::io("reobserve move source", e))?,
        ) != entry.modified
    {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(())
}
fn verify_copy(
    root: &Directory,
    manifest: &RelocationManifest,
    cancel: &WorkCancellation,
) -> Result<()> {
    let actual = RelocationManifest::capture(root, cancel)?;
    let expected = manifest
        .entries
        .iter()
        .filter(|e| !control(e))
        .collect::<Vec<_>>();
    let observed = actual
        .entries
        .iter()
        .filter(|e| !control(e))
        .collect::<Vec<_>>();
    if expected.len() != observed.len()
        || expected.iter().zip(observed).any(|(a, b)| {
            a.path != b.path
                || a.directory != b.directory
                || a.bytes != b.bytes
                || a.sha256 != b.sha256
        })
    {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(())
}
// Every deletion is qualified again by actual object and complete current
// bytes. A partial file, extra name, changed content or nonempty directory stays.
fn cleanup_tree(
    root: &Directory,
    manifest: &RelocationManifest,
    record: Option<(&str, Option<&Record>)>,
    retired: bool,
    acknowledge: impl FnOnce() -> Result<()>,
) -> Result<CleanupReport> {
    let cancel = WorkCancellation::default();
    let mut report = CleanupReport::default();
    if !retired {
        for entry in manifest.entries.iter().rev().filter(|e| {
            !control(e) && e.path.as_slice() != [crate::identity::IDENTITY_FILE.as_bytes().to_vec()]
        }) {
            let result = (|| -> Result<()> {
                let (parent, name) = parent_for(root, entry)?;
                if entry.directory {
                    let child = parent.child(&name)?;
                    if child.identity() != entry.identity {
                        return Err(StorageError::SnapshotChanged);
                    }
                    parent.remove_empty_child(&name, &child)
                } else {
                    let mut file = parent.open_regular(&name, rustix::fs::OFlags::RDONLY)?;
                    if FileIdentity::from_file(&file)? != entry.identity
                        || !file_matches(&mut file, entry, &cancel)?
                    {
                        return Err(StorageError::SnapshotChanged);
                    }
                    parent.remove_file_matching(&name, &file)
                }
            })();
            if let Err(error) = result
                && !matches!(&error,StorageError::Io {source,..} if source.kind()==std::io::ErrorKind::NotFound)
            {
                report.pending = true;
                report.preserved_files += 1;
            }
        }
    }
    // Keep all identity/control witnesses when anything remains. Cleanup can
    // then be reconciled explicitly, including a disconnected source drive.
    if report.pending {
        return Ok(report);
    }
    let controls = [
        crate::identity::IDENTITY_FILE,
        NATIVE_STATE_FILE,
        crate::platform::lock::LIBRARY_LOCK_FILE,
    ];
    let names = root.entries(controls.len() + 1, &cancel)?;
    if names
        .iter()
        .any(|name| !controls.iter().any(|c| name == OsStr::new(c)))
    {
        return Ok(CleanupReport {
            pending: true,
            preserved_files: 1,
        });
    }
    let mut checked = Vec::new();
    for name in controls {
        let result = (|| -> Result<Option<File>> {
            let mut file = match root.read_regular(OsStr::new(name))? {
                Some(file) => file,
                None if retired => return Ok(None),
                None => return Err(StorageError::SnapshotChanged),
            };
            let entry = manifest
                .entries
                .iter()
                .find(|e| e.path == [name.as_bytes().to_vec()])
                .ok_or(StorageError::IdentityMismatch)?;
            if FileIdentity::from_file(&file)? != entry.identity {
                return Err(StorageError::SnapshotChanged);
            }
            if name == NATIVE_STATE_FILE {
                let mut bytes = Vec::new();
                (&mut file)
                    .take((crate::profile::MAX_PROFILE_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|e| StorageError::io("check cleanup state", e))?;
                if record.is_none_or(|(hash, _)| digest(&bytes) != hash) {
                    return Err(StorageError::SnapshotChanged);
                }
            } else if !file_matches(&mut file, entry, &cancel)? {
                return Err(StorageError::IdentityMismatch);
            }
            Ok(Some(file))
        })();
        match result {
            Ok(Some(file)) => checked.push((name, file)),
            Ok(None) => (),
            Err(_) => {
                return Ok(CleanupReport {
                    pending: true,
                    preserved_files: 1,
                });
            }
        }
    }
    if !retired {
        acknowledge()?;
    }
    for (name, file) in checked {
        if root.remove_file_matching(OsStr::new(name), &file).is_err() {
            report.pending = true;
            report.preserved_files += 1;
            break;
        }
    }
    Ok(report)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveredRoot {
    Source,
    Destination,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelocationRecovery {
    pub authoritative: RecoveredRoot,
    pub cleanup: CleanupReport,
}
/// Reconcile a native intent before ordinary open. Actual namespace/root locks,
/// raw identity and native control proof qualify the profile-selected root.
/// An unavailable committed destination NEVER falls back to the source.
pub fn recover_relocation(
    store: &NativeProfileStore,
    locations: &ProfileLocations,
    cancel: &WorkCancellation,
) -> Result<Option<RelocationRecovery>> {
    let Some(bytes) = store.read_move_journal()? else {
        return Ok(None);
    };
    let (intent, retired) = Intent::decode_journal(&bytes)?;
    if cancel.is_cancelled() {
        return Err(StorageError::Cancelled);
    }
    namespace_roots(store, locations)?;
    if store.marker_digest()? != intent.profile_sha256 {
        return Err(StorageError::RecoveryBindingRequired);
    }
    let document = store.load()?.ok_or(StorageError::RecoveryBindingRequired)?;
    let profile = document.profile();
    if !profile.complete || profile.recovering || profile.library_id != intent.raw_id {
        return Err(StorageError::RecoveryBindingRequired);
    }
    let committed = if profile.root == intent.source {
        false
    } else if profile.root == intent.target {
        true
    } else {
        return Err(StorageError::RecoveryBindingRequired);
    };
    if retired && !committed {
        return Err(StorageError::RecoveryBindingRequired);
    }
    let path = Path::new(&profile.root);
    if fs::canonicalize(path)
        .map_err(|e| StorageError::io("resolve authoritative recovery root", e))?
        != path
    {
        return Err(StorageError::RecoveryBindingRequired);
    }
    let root = Directory::open(path)?;
    if root.identity()
        != if committed {
            intent.workspace_directory
        } else {
            intent.source_directory
        }
    {
        return Err(StorageError::RecoveryBindingRequired);
    }
    let lease = LibraryLease::acquire_existing_in(path, &root)?;
    let record = Record::open(&root)?;
    let d = &record.document;
    if d.library_root != profile.root
        || d.library_id_raw != intent.raw_id
        || d.profile_marker_sha256 != intent.profile_sha256
        || d.native_profile_root
            != exact_path(
                &fs::canonicalize(store.directory())
                    .map_err(|e| StorageError::io("resolve recovery namespace", e))?,
            )?
        || d.native_data_root
            != exact_path(
                &fs::canonicalize(&locations.data)
                    .map_err(|e| StorageError::io("resolve recovery data", e))?,
            )?
    {
        return Err(StorageError::RecoveryBindingRequired);
    }
    let mut marker = root.open_regular(
        OsStr::new(crate::identity::IDENTITY_FILE),
        rustix::fs::OFlags::RDONLY,
    )?;
    let identity = LibraryIdentity::from_bytes(read_marker(&mut marker)?)?;
    identity.require_spelling(&intent.raw_id)?;
    if digest(identity.original_bytes()) != d.library_marker_sha256 {
        return Err(StorageError::RecoveryBindingRequired);
    }
    if let Some(policy) = &d.root_policy {
        require_explicit_paths(path, Path::new(&d.source_root), policy, store, locations)?;
    } else {
        require_reopened_paths(path, Path::new(&d.source_root), store, locations)?;
    }
    if committed
        && d.root_policy.as_ref().and_then(|p| p.relocation.as_ref()) != Some(&intent.proof())
    {
        return Err(StorageError::RecoveryBindingRequired);
    }
    require_root_entries(&root, d.version == 2)?;
    store.admit_move_journal(&bytes)?;
    let mut cleanup = CleanupReport {
        pending: true,
        preserved_files: 0,
    };
    if committed {
        // Source disconnection/permission failures keep cleanup pending. Only
        // the profile-selected destination may become the running library.
        let result = (|| -> Result<CleanupReport> {
            verify_copy(&root, &intent.manifest, cancel)?;
            require_no_sidecars(&root.child(OsStr::new("db"))?)?;
            let scratch = tempfile::tempdir()
                .map_err(|e| StorageError::io("create recovery integrity probe", e))?;
            crate::backup::probe_owned(&path.join("db/librairii.sqlite3"), scratch.path())?;
            let source_path = Path::new(&intent.source);
            let source = Directory::open(source_path)?;
            if source.identity() != intent.source_directory {
                return Err(StorageError::SnapshotChanged);
            }
            let _source_lease = if source
                .read_regular(OsStr::new(crate::platform::lock::LIBRARY_LOCK_FILE))?
                .is_some()
            {
                Some(LibraryLease::acquire_existing_in(source_path, &source)?)
            } else if retired {
                None
            } else {
                return Err(StorageError::RecoveryBindingRequired);
            };
            if !retired {
                let source_record = Record::open(&source)?;
                if digest(&source_record.bytes) != intent.source_state_sha256 {
                    return Err(StorageError::SnapshotChanged);
                }
            }
            let mut report = cleanup_tree(
                &source,
                &intent.manifest,
                Some((&intent.source_state_sha256, None)),
                retired,
                || store.acknowledge_move_retirement(&intent.bytes()?),
            )?;
            if !report.pending {
                let parent =
                    Directory::open(source_path.parent().ok_or(StorageError::RootNotAbsolute)?)?;
                if parent
                    .remove_empty_child(
                        source_path
                            .file_name()
                            .ok_or(StorageError::RootNotAbsolute)?,
                        &source,
                    )
                    .is_err()
                {
                    report.pending = true;
                }
            }
            Ok(report)
        })();
        if let Ok(report) = result {
            cleanup = report;
        }
        // A durable retirement receipt proves all old product data was
        // retired before any control deletion. A disconnected/changed skeleton
        // is preserved, but cannot permanently block a later library move.
        if !cleanup.pending || retired {
            store.retire_move_journal()?;
        }
    }
    lease.revalidate()?;
    record.revalidate(&root)?;
    Ok(Some(RelocationRecovery {
        authoritative: if committed {
            RecoveredRoot::Destination
        } else {
            RecoveredRoot::Source
        },
        cleanup,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    const MOVE: &str = "c0000000-0000-4000-8000-000000000013";
    fn close(
        native: NativeLibrary<'_>,
    ) -> std::result::Result<ClosedNativeLibrary<'_>, Box<dyn std::error::Error>> {
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native
            .close_for_relocation()
            .map_err(|f| f.to_string().into())
    }
    fn finish(native: NativeLibrary<'_>) -> TestResult {
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string().into())
    }
    #[test]
    fn relocation_streams_complete_tree_commits_exact_root_and_reopens() -> TestResult {
        for existing in [false, true] {
            let (roots, copy, locations) = crate::test_support::native_copy()?;
            let store = NativeProfileStore::create_fresh(&locations.native)?;
            let native =
                NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
                    .map_err(|f| f.to_string())?;
            let old = native.layout().root().to_path_buf();
            let closed = close(native)?;
            let target = roots.library().join("chosen destination");
            if existing {
                fs::create_dir(&target)?;
            }
            let preview = TargetPreview::inspect(&target, &[], &Default::default())?;
            let outcome = closed
                .relocate(preview, &locations, MOVE.parse()?, &Default::default())
                .map_err(|f| f.to_string())?;
            assert!(!outcome.cleanup.pending);
            assert!(!old.exists());
            assert_eq!(outcome.library.layout().root(), target);
            assert_eq!(
                store.load()?.ok_or("profile")?.profile().root,
                target.to_str().ok_or("root")?
            );
            assert!(
                !locations
                    .native
                    .join(crate::profile::relocation::JOURNAL)
                    .exists()
            );
            let mut db = Database::open_native(&outcome.library)?;
            assert!(
                db.read(|c| c
                    .query_row("SELECT COUNT(*) FROM stories", [], |r| r.get::<_, i64>(0))
                    .map_err(StorageError::from))?
                    .wait()?
                    > 0
            );
            db.close()?;
            drop(db);
            outcome.library.finish().map_err(|f| f.to_string())?;
            let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
            finish(native)?;
        }
        Ok(())
    }
    #[test]
    fn before_commit_failure_keeps_source_authority_and_retry_preserves_new_language() -> TestResult
    {
        let (roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let old = native.layout().root().to_path_buf();
        let closed = close(native)?;
        let target = roots.library().join("retry destination");
        let preview = TargetPreview::inspect(&target, &[], &Default::default())?;
        let attempt = Attempt {
            publication_uncertain: false,
            source: Some(closed),
            destination: None,
            target: preview,
            locations: locations.clone(),
            id: MOVE.parse()?,
            workspace: None,
            intent: None,
            journal: false,
            parents: Vec::new(),
        };
        let failure = attempt
            .complete(&Default::default(), |point| {
                if point == Point::BeforeCommit {
                    Err(StorageError::Cancelled)
                } else {
                    Ok(())
                }
            })
            .err()
            .ok_or("fault")?;
        assert_eq!(failure.commit_status(), CommitStatus::BeforeCommit);
        assert_eq!(
            store.load()?.ok_or("profile")?.profile().root,
            old.to_str().ok_or("root")?
        );
        store.update(|p| {
            p.language = "en".into();
            Ok(())
        })?;
        let outcome = failure
            .retry(&Default::default())
            .map_err(|f| f.to_string())?;
        assert_eq!(store.load()?.ok_or("profile")?.profile().language, "en");
        finish(outcome.library)?;
        Ok(())
    }
    #[test]
    fn committed_failure_recovers_destination_and_preserves_changed_and_new_source_files()
    -> TestResult {
        let (roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let old = native.layout().root().to_path_buf();
        let closed = close(native)?;
        let target = roots.library().join("committed destination");
        let preview = TargetPreview::inspect(&target, &[], &Default::default())?;
        let attempt = Attempt {
            publication_uncertain: false,
            source: Some(closed),
            destination: None,
            target: preview,
            locations: locations.clone(),
            id: MOVE.parse()?,
            workspace: None,
            intent: None,
            journal: false,
            parents: Vec::new(),
        };
        let failure = attempt
            .complete(&Default::default(), |point| {
                if point == Point::Committed {
                    Err(StorageError::Cancelled)
                } else {
                    Ok(())
                }
            })
            .err()
            .ok_or("fault")?;
        assert_eq!(failure.commit_status(), CommitStatus::Committed);
        fs::write(old.join("catalog/new-after-commit"), b"keep")?;
        fs::write(old.join("logs/changed"), b"keep too")?;
        // Dropping the interrupted process's owners releases OS exclusion only.
        drop(failure);
        drop(store);
        let store = NativeProfileStore::open_native(&locations.native)?;
        let recovery =
            recover_relocation(&store, &locations, &Default::default())?.ok_or("journal")?;
        assert_eq!(recovery.authoritative, RecoveredRoot::Destination);
        assert!(recovery.cleanup.pending);
        assert_eq!(fs::read(old.join("catalog/new-after-commit"))?, b"keep");
        let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
        finish(native)?;
        assert_eq!(fs::read(old.join("logs/changed"))?, b"keep too");
        Ok(())
    }
    #[test]
    fn crash_before_commit_and_corrupt_journal_never_replace_source_or_follow_foreign_paths()
    -> TestResult {
        let (roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let old = native.layout().root().to_path_buf();
        let closed = close(native)?;
        let target = roots.library().join("interrupted");
        let attempt = Attempt {
            publication_uncertain: false,
            source: Some(closed),
            destination: None,
            target: TargetPreview::inspect(&target, &[], &Default::default())?,
            locations: locations.clone(),
            id: MOVE.parse()?,
            workspace: None,
            intent: None,
            journal: false,
            parents: Vec::new(),
        };
        let failure = attempt
            .complete(&Default::default(), |point| {
                if point == Point::File {
                    Err(StorageError::Cancelled)
                } else {
                    Ok(())
                }
            })
            .err()
            .ok_or("fault")?;
        drop(failure);
        drop(store);
        let store = NativeProfileStore::open_native(&locations.native)?;
        let recovery =
            recover_relocation(&store, &locations, &Default::default())?.ok_or("journal")?;
        assert_eq!(recovery.authoritative, RecoveredRoot::Source);
        assert!(recovery.cleanup.pending);
        let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
        finish(native)?;
        drop(store);
        let before = fs::read(old.join("db/librairii.sqlite3"))?;
        fs::write(
            locations.native.join(crate::profile::relocation::JOURNAL),
            b"{\"version\":999}",
        )?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        assert!(recover_relocation(&store, &locations, &Default::default()).is_err());
        assert!(NativeLibrary::reopen(&store, &locations, &Default::default()).is_err());
        assert_eq!(fs::read(old.join("db/librairii.sqlite3"))?, before);
        Ok(())
    }
}

#[derive(Debug)]
pub enum RestartMoveFailure<'profile> {
    Recovery(StorageError),
    Move(Box<RelocationFailure<'profile>>),
}
impl std::fmt::Display for RestartMoveFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recovery(e) => e.fmt(f),
            Self::Move(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for RestartMoveFailure<'_> {}
/// Retry the actual incomplete owned destination after a process restart.
/// Complete files are hash-verified; partial/changed files are preserved and
/// refuse retry. Cancel can then keep that workspace and release the journal.
pub fn retry_relocation<'profile>(
    store: &'profile NativeProfileStore,
    locations: &ProfileLocations,
    cancel: &WorkCancellation,
) -> std::result::Result<RelocatedLibrary<'profile>, RestartMoveFailure<'profile>> {
    let prepared = (|| -> Result<(Intent, Workspace, TargetPreview)> {
        let recovery = recover_relocation(store, locations, cancel)?
            .ok_or(StorageError::RecoveryBindingRequired)?;
        if recovery.authoritative != RecoveredRoot::Source {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let bytes = store
            .read_move_journal()?
            .ok_or(StorageError::RecoveryBindingRequired)?;
        let intent = Intent::decode(&bytes)?;
        let target = Path::new(&intent.target);
        let (path, published) = match Directory::open(target) {
            Ok(root) if root.identity() == intent.workspace_directory => {
                (target, !intent.selected_workspace)
            }
            _ => (Path::new(&intent.workspace), false),
        };
        let root = Directory::open(path)?;
        if root.identity() != intent.workspace_directory {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let lease = LibraryLease::acquire_existing_in(path, &root)?;
        let record = Record::open(&root)?;
        if record.document.library_root != intent.target
            || record.document.library_id_raw != intent.raw_id
            || record.document.profile_marker_sha256 != intent.profile_sha256
            || record
                .document
                .root_policy
                .as_ref()
                .and_then(|p| p.relocation.as_ref())
                != Some(&intent.proof())
            || record.document.state != Phase::Closed
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let parent_path = target
            .parent()
            .ok_or(StorageError::RootNotAbsolute)?
            .to_path_buf();
        let parent = Directory::open(&parent_path)?;
        let name = EntryName::parse(
            Path::new(&intent.workspace)
                .file_name()
                .ok_or(StorageError::SettingsDestination)?,
        )?;
        let target_name = EntryName::parse(
            target
                .file_name()
                .ok_or(StorageError::SettingsDestination)?,
        )?;
        let preview = TargetPreview::recovery_observation(target)?;
        Ok((
            intent,
            Workspace {
                parent,
                parent_path,
                name,
                target_name,
                root,
                document: record.document.clone(),
                record: Some(record),
                lease: Some(lease),
                allocation: None,
                lease_attempted: true,
                published,
            },
            preview,
        ))
    })()
    .map_err(RestartMoveFailure::Recovery)?;
    let (intent, workspace, target) = prepared;
    let native =
        NativeLibrary::reopen(store, locations, cancel).map_err(RestartMoveFailure::Recovery)?;
    Attempt {
        publication_uncertain: false,
        source: Some(ClosedNativeLibrary { library: native }),
        destination: None,
        target,
        locations: locations.clone(),
        id: intent
            .id
            .parse()
            .map_err(|_| RestartMoveFailure::Recovery(StorageError::RecoveryBindingRequired))?,
        workspace: Some(workspace),
        intent: Some(intent),
        journal: true,
        parents: Vec::new(),
    }
    .complete(cancel, |_| Ok(()))
    .map_err(RestartMoveFailure::Move)
}
/// Explicit cancellation before commitment. Preserve any unproven destination
/// bytes, retire only the verified native intent, and permit another move.
/// The caller displays the returned pending-cleanup report and retained path.
pub fn cancel_relocation(
    store: &NativeProfileStore,
    locations: &ProfileLocations,
    cancel: &WorkCancellation,
) -> Result<(RelocationRecovery, PathBuf)> {
    let recovery = recover_relocation(store, locations, cancel)?
        .ok_or(StorageError::RecoveryBindingRequired)?;
    if recovery.authoritative != RecoveredRoot::Source {
        return Err(StorageError::RecoveryBindingRequired);
    }
    let intent = Intent::decode(
        &store
            .read_move_journal()?
            .ok_or(StorageError::RecoveryBindingRequired)?,
    )?;
    let retained = match Directory::open(Path::new(&intent.target)) {
        Ok(root) if root.identity() == intent.workspace_directory => PathBuf::from(&intent.target),
        _ => PathBuf::from(&intent.workspace),
    };
    store.retire_move_journal()?;
    Ok((recovery, retained))
}
