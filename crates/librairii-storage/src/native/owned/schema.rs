//! Explicit foreign-schema recovery. The public wrapper never exposes its
//! schema-unverified native owner. All original DB/WAL/SHM bytes are copied,
//! verified and durably receipted before any original name can be retired.
//! A receipt is data: replay still acquires the real root/profile leases and
//! verifies every actual directory, file, digest and remaining original name.
use super::*;
use crate::platform::hash_stream;
use crate::platform::unix::{ConfigTemp, ControlCreateFailure, FileIdentity, durable_file_flush};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    io::{Seek, SeekFrom, Write},
    sync::atomic::AtomicU64,
};
const FAMILY: [&str; 3] = [
    "librairii.sqlite3",
    "librairii.sqlite3-wal",
    "librairii.sqlite3-shm",
];
const PREFIX: &str = "schema-conflict-recovery-";
const RECEIPT: &str = "preserved.json";
const FINISHED: &str = "replacement-created.json";
static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaPreview {
    Foreign,
    ResumePreserved,
    FinishReplacement,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    name: String,
    bytes: u64,
    sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    library_id: String,
    members: Vec<Member>,
}
#[derive(Debug)]
struct Source {
    file: File,
    member: Member,
    identity: FileIdentity,
    modified: [i64; 4],
}
#[derive(Debug)]
struct Archive {
    name: OsString,
    directory: Option<Directory>,
    manifest: Option<Manifest>,
    receipt: Option<ConfigTemp>,
    finished: Option<ConfigTemp>,
    copies: Vec<ConfigTemp>,
    allocation: Option<Box<ControlCreateFailure>>,
}
/// The exact root/profile and all failed allocations remain owned. An error
/// never becomes a retry instruction or a claim of successful database close.
#[derive(Debug)]
pub struct SchemaRecovery<'p> {
    native: NativeLibrary<'p>,
    preview: Option<SchemaPreview>,
    error: Option<String>,
    sources: Vec<Source>,
    archives: Vec<Archive>,
}
#[derive(Debug)]
pub struct SchemaRecoveryFailure<'p> {
    recovery: SchemaRecovery<'p>,
    cause: StorageError,
}
impl std::fmt::Display for SchemaRecoveryFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for SchemaRecoveryFailure<'_> {}
impl SchemaRecoveryFailure<'_> {
    pub fn root(&self) -> &Path {
        self.recovery.root()
    }
    pub fn retained_paths(&self) -> Vec<PathBuf> {
        self.recovery.retained_paths()
    }
}
impl<'p> SchemaRecovery<'p> {
    /// Read-only conflict observation; the independent SQLite probe never opens
    /// the originals. Only this wrapper can retain the schema-unverified owner.
    pub fn open(
        store: &'p NativeProfileStore,
        locations: &ProfileLocations,
        cancel: &WorkCancellation,
    ) -> Result<Self> {
        let native = NativeLibrary::reopen_mode(store, locations, cancel, ReopenMode::Schema)?;
        Ok(Self::from_native(native))
    }
    pub(super) fn pending(native: &NativeLibrary<'_>) -> Result<bool> {
        native.revalidate_database_ownership()?;
        Ok(!archives(native)?.is_empty())
    }
    pub(super) fn from_native(native: NativeLibrary<'p>) -> Self {
        let mut recovery = Self {
            native,
            preview: None,
            error: None,
            sources: Vec::new(),
            archives: Vec::new(),
        };
        match recovery.inspect() {
            Ok(preview) => recovery.preview = Some(preview),
            Err(error) => recovery.error = Some(error.to_string()),
        }
        recovery
    }
    pub fn root(&self) -> &Path {
        self.native.layout.root()
    }
    pub fn preview(&self) -> Option<SchemaPreview> {
        self.preview
    }
    /// Quit a read-only recovery observation. This writes no Closed ACK and
    /// grants no reusable native owner. Confirm consumes this type; its failed
    /// allocations/writes are held by SchemaRecoveryFailure instead.
    pub fn abandon_observation(self) {
        drop(self);
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn retained_paths(&self) -> Vec<PathBuf> {
        self.archives
            .iter()
            .map(|a| self.native.layout.root().join("db").join(&a.name))
            .collect()
    }
    fn inspect(&mut self) -> Result<SchemaPreview> {
        self.native.revalidate(&WorkCancellation::default())?;
        if !self
            .native
            .record
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?
            .document
            .temporaries
            .is_empty()
        {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        require_settled_trash(&self.native.root)?;
        self.sources = family(&self.native.database)?;
        self.archives = archives(&self.native)?;
        let preview = if self.sources.iter().any(|s| s.member.name == FAMILY[0]) {
            let scratch = tempfile::tempdir()
                .map_err(|e| StorageError::io("create independent schema probe", e))?;
            match crate::backup::probe_owned(&self.native.layout.database(), scratch.path()) {
                Err(StorageError::ForeignSchema) => SchemaPreview::Foreign,
                Ok(_) if !self.archives.is_empty() => SchemaPreview::FinishReplacement,
                Ok(_) => return Err(StorageError::NoSchemaConflict),
                Err(error) => return Err(error),
            }
        } else if self
            .archives
            .iter()
            .any(|a| remaining_match(a, &self.sources))
        {
            SchemaPreview::ResumePreserved
        } else {
            return Err(StorageError::CustodyRecoveryRequired);
        };
        verify_family(&self.native.database, &mut self.sources)?;
        Ok(preview)
    }
    /// After explicit UI confirmation only. A failed actual DB close stays on
    /// this stack forever and continues notifying the caller; it is never
    /// represented by a reconstructed database or a successful owner drop.
    pub fn confirm(
        mut self,
        mut unclosed: impl FnMut(&StorageError, &Path),
    ) -> std::result::Result<(NativeLibrary<'p>, Vec<PathBuf>), Box<SchemaRecoveryFailure<'p>>>
    {
        match self.confirm_inner(&mut unclosed) {
            Ok(()) => {
                let paths = self.retained_paths();
                Ok((self.native, paths))
            }
            Err(cause) => Err(Box::new(SchemaRecoveryFailure {
                recovery: self,
                cause,
            })),
        }
    }
    fn confirm_inner(&mut self, unclosed: &mut impl FnMut(&StorageError, &Path)) -> Result<()> {
        let preview = self.preview.ok_or(StorageError::NoSchemaConflict)?;
        self.native.revalidate(&WorkCancellation::default())?;
        verify_family(&self.native.database, &mut self.sources)?;
        for archive in &mut self.archives {
            verify_archive(&self.native, archive)?;
        }
        // This is an actual exclusive native writer claim with durable Active
        // ACK. No Database runtime, pool or job owner exists on this path yet.
        if preview != SchemaPreview::FinishReplacement {
            self.native.claim_runtime()?;
            let selected = self
                .archives
                .iter()
                .position(|a| remaining_match(a, &self.sources));
            let index = if let Some(index) = selected {
                index
            } else {
                self.create_archive()?;
                self.archives.len() - 1
            };
            verify_family(&self.native.database, &mut self.sources)?;
            verify_archive(&self.native, &mut self.archives[index])?;
            checkpoint(0)?;
            for source in &mut self.sources {
                verify_archive(&self.native, &mut self.archives[index])?;
                verify_source(&self.native.database, source)?;
                self.native.revalidate_database_ownership()?;
                self.native
                    .database
                    .remove_file_matching(OsStr::new(&source.member.name), &source.file)?;
                if source.member.name == FAMILY[0] {
                    checkpoint(1)?;
                }
            }
            require_no_sidecars(&self.native.database)?;
            // This method has positively drained only its own synchronous file
            // work. It grants no Closed proof: SQLite must actually open and close.
            self.native.runtime_active.store(false, Ordering::Release);
        }
        self.sources.clear();
        self.native.revalidate_database_ownership()?;
        {
            let mut database = crate::database::Database::open_native(&self.native)?;
            if let Err(error) = database.close() {
                loop {
                    unclosed(&error, self.native.layout.root());
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
        if !self.native.closed_success.load(Ordering::Acquire) {
            return Err(StorageError::RuntimeFenced);
        }
        checkpoint(2)?;
        for archive in &mut self.archives {
            verify_archive(&self.native, archive)?;
            let bytes = serde_json::to_vec(
                archive
                    .manifest
                    .as_ref()
                    .ok_or(StorageError::CustodyRecoveryRequired)?,
            )
            .map_err(|_| StorageError::CustodyRecoveryRequired)?;
            let directory = archive
                .directory
                .as_ref()
                .ok_or(StorageError::CustodyRecoveryRequired)?;
            record(
                directory,
                &mut archive.finished,
                &mut archive.allocation,
                FINISHED,
                &bytes,
            )?;
            directory.flush()?;
            self.native.database.flush()?;
        }
        Ok(())
    }
    fn create_archive(&mut self) -> Result<()> {
        let name = OsString::from(format!(
            "{PREFIX}{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        self.archives.push(Archive {
            name: name.clone(),
            directory: None,
            manifest: None,
            receipt: None,
            finished: None,
            copies: Vec::new(),
            allocation: None,
        });
        let archive = self
            .archives
            .last_mut()
            .ok_or(StorageError::RuntimeFenced)?;
        archive.directory = Some(self.native.database.create_child(&name)?);
        let directory = archive
            .directory
            .as_ref()
            .ok_or(StorageError::RuntimeFenced)?;
        for source in &mut self.sources {
            verify_source(&self.native.database, source)?;
            match directory.create_control_record(OsStr::new(&source.member.name)) {
                Ok(copy) => archive.copies.push(copy),
                Err(failure) => {
                    archive.allocation = Some(failure);
                    return Err(StorageError::CustodyRecoveryRequired);
                }
            }
            let copy = archive
                .copies
                .last_mut()
                .ok_or(StorageError::RuntimeFenced)?;
            source
                .file
                .seek(SeekFrom::Start(0))
                .map_err(|e| StorageError::io("rewind schema source", e))?;
            let copied = std::io::copy(
                &mut (&mut source.file).take(source.member.bytes.saturating_add(1)),
                &mut copy.file,
            )
            .map_err(|e| StorageError::io("copy preserved schema member", e))?;
            if copied != source.member.bytes {
                return Err(StorageError::SnapshotChanged);
            }
            durable_file_flush(&copy.file)?;
            if measure(&mut copy.file, &source.member.name)? != source.member {
                return Err(StorageError::IntegrityFailed);
            }
        }
        verify_family(&self.native.database, &mut self.sources)?;
        let manifest = Manifest {
            version: 1,
            library_id: self.native.identity.spelling().into(),
            members: self.sources.iter().map(|s| s.member.clone()).collect(),
        };
        let bytes =
            serde_json::to_vec(&manifest).map_err(|_| StorageError::CustodyRecoveryRequired)?;
        archive.manifest = Some(manifest);
        directory.flush()?;
        self.native.database.flush()?;
        record(
            directory,
            &mut archive.receipt,
            &mut archive.allocation,
            RECEIPT,
            &bytes,
        )?;
        directory.flush()?;
        self.native.database.flush()
    }
}
#[cfg(not(test))]
fn checkpoint(_: u8) -> Result<()> {
    Ok(())
}
#[cfg(test)]
fn checkpoint(point: u8) -> Result<()> {
    let Some(raw) = std::env::var_os("LIBRAIRII_SCHEMA_RECOVERY_CRASH") else {
        return Ok(());
    };
    let context: serde_json::Value =
        serde_json::from_slice(raw.as_encoded_bytes()).map_err(|_| StorageError::InvalidProfile)?;
    if context["point"].as_u64() != Some(u64::from(point)) {
        return Ok(());
    }
    let ready = Path::new(
        context["ready"]
            .as_str()
            .ok_or(StorageError::InvalidProfile)?,
    );
    let parent = ready.parent().ok_or(StorageError::InvalidProfile)?;
    if !ready.is_absolute()
        || fs::read(parent.join(".schema-recovery-fixture"))
            .map_err(|e| StorageError::io("read owned schema fixture marker", e))?
            != b"schema-recovery-v1\n"
    {
        return Err(StorageError::InvalidProfile);
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(ready)
        .map_err(|e| StorageError::io("record schema crash boundary", e))?;
    file.write_all(&[point])
        .map_err(|e| StorageError::io("write schema crash boundary", e))?;
    durable_file_flush(&file)?;
    Directory::open(parent)?.flush()?;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn record(
    directory: &Directory,
    target: &mut Option<ConfigTemp>,
    allocation: &mut Option<Box<ControlCreateFailure>>,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    match directory.create_control_record(OsStr::new(name)) {
        Ok(file) => *target = Some(file),
        Err(failure) => {
            *allocation = Some(failure);
            return Err(StorageError::CustodyRecoveryRequired);
        }
    }
    let file = &mut target.as_mut().ok_or(StorageError::RuntimeFenced)?.file;
    file.write_all(bytes)
        .map_err(|e| StorageError::io("write schema preservation receipt", e))?;
    durable_file_flush(file)
}
fn modified(file: &File) -> Result<[i64; 4]> {
    let m = file
        .metadata()
        .map_err(|e| StorageError::io("inspect schema member", e))?;
    if !m.is_file() || m.nlink() != 1 {
        return Err(StorageError::PathEscape);
    }
    Ok([m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()])
}
fn measure(file: &mut File, name: &str) -> Result<Member> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| StorageError::io("rewind schema member", e))?;
    let bytes = file
        .metadata()
        .map_err(|e| StorageError::io("measure schema member", e))?
        .len();
    let digest = hash_stream(file, bytes, &WorkCancellation::default())?;
    Ok(Member {
        name: name.into(),
        bytes,
        sha256: digest.checksum().to_string(),
    })
}
fn family(database: &Directory) -> Result<Vec<Source>> {
    let mut result = Vec::new();
    for name in FAMILY {
        if let Some(mut file) = database.read_regular(OsStr::new(name))? {
            let stamp = modified(&file)?;
            let identity = FileIdentity::from_file(&file)?;
            let member = measure(&mut file, name)?;
            if modified(&file)? != stamp {
                return Err(StorageError::SnapshotChanged);
            }
            result.push(Source {
                file,
                member,
                identity,
                modified: stamp,
            });
        }
    }
    Ok(result)
}
fn verify_source(database: &Directory, source: &mut Source) -> Result<()> {
    let named =
        database.open_regular(OsStr::new(&source.member.name), rustix::fs::OFlags::RDONLY)?;
    if FileIdentity::from_file(&named)? != source.identity
        || modified(&source.file)? != source.modified
        || measure(&mut source.file, &source.member.name)? != source.member
        || modified(&source.file)? != source.modified
    {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(())
}
fn verify_family(database: &Directory, sources: &mut [Source]) -> Result<()> {
    for name in FAMILY {
        if database.regular_entry(OsStr::new(name))?
            != sources.iter().any(|s| s.member.name == name)
        {
            return Err(StorageError::SnapshotChanged);
        }
    }
    for source in sources {
        verify_source(database, source)?;
    }
    Ok(())
}
fn remaining_match(archive: &Archive, sources: &[Source]) -> bool {
    archive
        .manifest
        .as_ref()
        .is_some_and(|m| sources.iter().all(|s| m.members.contains(&s.member)))
}
fn read_receipt(directory: &Directory, name: &str) -> Result<Option<ConfigTemp>> {
    if !directory.regular_entry(OsStr::new(name))? {
        return Ok(None);
    }
    Ok(Some(directory.read_config(OsStr::new(name))?))
}
fn decode(receipt: &mut ConfigTemp) -> Result<Manifest> {
    let mut bytes = Vec::new();
    receipt
        .file
        .seek(SeekFrom::Start(0))
        .map_err(|e| StorageError::io("rewind schema receipt", e))?;
    (&mut receipt.file)
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| StorageError::io("read schema preservation receipt", e))?;
    if bytes.len() > 16 * 1024 {
        return Err(StorageError::CustodyRecoveryRequired);
    }
    let m: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| StorageError::CustodyRecoveryRequired)?;
    let mut names = std::collections::BTreeSet::new();
    if m.version != 1
        || m.members.is_empty()
        || m.members.len() > 3
        || !m.members.iter().any(|m| m.name == FAMILY[0])
        || m.members.iter().any(|m| {
            !FAMILY.contains(&m.name.as_str())
                || !names.insert(&m.name)
                || m.sha256.parse::<librairii_core::Checksum>().is_err()
        })
    {
        return Err(StorageError::CustodyRecoveryRequired);
    }
    Ok(m)
}
fn archives(native: &NativeLibrary<'_>) -> Result<Vec<Archive>> {
    let mut result = Vec::new();
    for name in native
        .database
        .entries(4096, &WorkCancellation::default())?
    {
        if !name.as_encoded_bytes().starts_with(PREFIX.as_bytes()) {
            continue;
        }
        let directory = native.database.child(&name)?;
        let Some(mut receipt) = read_receipt(&directory, RECEIPT)? else {
            continue;
        };
        let manifest = decode(&mut receipt)?;
        if manifest.library_id != native.identity.spelling() {
            return Err(StorageError::IdentityMismatch);
        }
        if let Some(mut finished) = read_receipt(&directory, FINISHED)? {
            if serde_json::to_vec(&decode(&mut finished)?)
                .map_err(|_| StorageError::CustodyRecoveryRequired)?
                != serde_json::to_vec(&manifest)
                    .map_err(|_| StorageError::CustodyRecoveryRequired)?
            {
                return Err(StorageError::CustodyRecoveryRequired);
            }
            continue;
        }
        let copies = manifest
            .members
            .iter()
            .map(|m| directory.read_config(OsStr::new(&m.name)))
            .collect::<Result<Vec<_>>>()?;
        let mut archive = Archive {
            name,
            directory: Some(directory),
            manifest: Some(manifest),
            receipt: Some(receipt),
            finished: None,
            copies,
            allocation: None,
        };
        verify_archive(native, &mut archive)?;
        result.push(archive);
        if result.len() > 64 {
            return Err(StorageError::CustodyRecoveryRequired);
        }
    }
    Ok(result)
}
fn verify_archive(native: &NativeLibrary<'_>, archive: &mut Archive) -> Result<()> {
    native.revalidate_database_ownership()?;
    let directory = archive
        .directory
        .as_ref()
        .ok_or(StorageError::CustodyRecoveryRequired)?;
    if native.database.child(&archive.name)?.identity() != directory.identity() {
        return Err(StorageError::SnapshotChanged);
    }
    let manifest = archive
        .manifest
        .as_ref()
        .ok_or(StorageError::CustodyRecoveryRequired)?;
    if manifest.library_id != native.identity.spelling() {
        return Err(StorageError::IdentityMismatch);
    }
    let mut named = directory.read_config(OsStr::new(RECEIPT))?;
    if serde_json::to_vec(&decode(&mut named)?)
        .map_err(|_| StorageError::CustodyRecoveryRequired)?
        != serde_json::to_vec(manifest).map_err(|_| StorageError::CustodyRecoveryRequired)?
    {
        return Err(StorageError::SnapshotChanged);
    }
    for copy in &mut archive.copies {
        let named = directory.open_regular(&copy.name, rustix::fs::OFlags::RDONLY)?;
        if FileIdentity::from_file(&named)? != FileIdentity::from_file(&copy.file)?
            || !manifest.members.contains(&measure(
                &mut copy.file,
                copy.name.to_str().ok_or(StorageError::PathEscape)?,
            )?)
        {
            return Err(StorageError::SnapshotChanged);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[expect(
        clippy::panic,
        reason = "An actual unclosed SQLite owner must fail the test instead of hanging its process"
    )]
    fn unclosed(error: &StorageError, _: &Path) {
        panic!("unexpected actual database close failure: {error}");
    }
    type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn setup() -> TestResult<(librairii_testkit::TestRoots, ProfileLocations, PathBuf)> {
        let (roots, copy, locations) = crate::test_support::native_copy()?;
        let root = copy.root().to_owned();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let mut db = crate::database::Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        fs::write(root.join("db/librairii.sqlite3"), b"foreign database bytes")?;
        fs::write(root.join("db/librairii.sqlite3-wal"), b"foreign WAL bytes")?;
        fs::write(root.join("db/librairii.sqlite3-shm"), b"foreign SHM bytes")?;
        Ok((roots, locations, root))
    }
    fn payloads(root: &Path) -> TestResult<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
        let mut result = std::collections::BTreeMap::new();
        let mut pending = vec![root.to_owned()];
        while let Some(path) = pending.pop() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    pending.push(entry.path());
                } else {
                    result.insert(
                        entry.path().strip_prefix(root)?.into(),
                        fs::read(entry.path())?,
                    );
                }
            }
        }
        Ok(result)
    }
    fn verify_preserved(path: &Path) -> TestResult {
        for (name, bytes) in FAMILY.into_iter().zip([
            b"foreign database bytes".as_slice(),
            b"foreign WAL bytes",
            b"foreign SHM bytes",
        ]) {
            assert_eq!(fs::read(path.join(name))?, bytes);
        }
        Ok(())
    }
    #[test]
    fn native_schema_confirmation_preserves_complete_family_and_archives() -> TestResult {
        let (_roots, locations, root) = setup()?;
        let before = payloads(&root.join("archives"))?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        assert!(NativeLibrary::reopen(&store, &locations, &Default::default()).is_err());
        let recovery = SchemaRecovery::open(&store, &locations, &Default::default())?;
        assert_eq!(recovery.preview(), Some(SchemaPreview::Foreign));
        assert!(recovery.retained_paths().is_empty());
        assert_eq!(
            fs::read(root.join("db/librairii.sqlite3-wal"))?,
            b"foreign WAL bytes"
        );
        store.update(|p| {
            p.language = "en".into();
            Ok(())
        })?;
        let (native, paths) = recovery.confirm(unclosed).map_err(|f| f.to_string())?;
        assert_eq!(paths.len(), 1);
        verify_preserved(&paths[0])?;
        assert_eq!(payloads(&root.join("archives"))?, before);
        assert_eq!(store.load()?.ok_or("profile")?.profile().language, "en");
        native.finish().map_err(|f| f.to_string())?;
        let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
        let mut db = crate::database::Database::open_native(&native)?;
        assert_eq!(
            db.read(|c| Ok(
                c.query_row("SELECT count(*) FROM stories", [], |r| r.get::<_, i64>(0))?
            ))?
            .wait()?,
            0
        );
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        Ok(())
    }
    #[test]
    fn native_schema_changed_preview_and_permission_failure_retain_actual_owner() -> TestResult {
        use std::os::unix::fs::PermissionsExt;
        for changed in [true, false] {
            let (_roots, locations, root) = setup()?;
            let store = NativeProfileStore::open_native(&locations.native)?;
            let recovery = SchemaRecovery::open(&store, &locations, &Default::default())?;
            if changed {
                fs::write(root.join("db/librairii.sqlite3"), b"changed after preview")?;
            } else {
                fs::set_permissions(root.join("db"), fs::Permissions::from_mode(0o500))?;
            }
            let Err(failure) = recovery.confirm(unclosed) else {
                return Err("unsafe schema replacement succeeded".into());
            };
            fs::set_permissions(root.join("db"), fs::Permissions::from_mode(0o700))?;
            assert_eq!(
                fs::read(root.join("db/librairii.sqlite3"))?,
                if changed {
                    b"changed after preview".as_slice()
                } else {
                    b"foreign database bytes"
                }
            );
            assert_eq!(
                fs::read(root.join("db/librairii.sqlite3-wal"))?,
                b"foreign WAL bytes"
            );
            assert!(LibraryLease::acquire_existing_in(&root, &Directory::open(&root)?).is_err());
            assert_eq!(failure.root(), root);
            if !changed {
                assert_eq!(failure.retained_paths().len(), 1);
            }
        }
        Ok(())
    }
    #[test]
    fn native_schema_replay_revalidates_durable_copies_after_process_death() -> TestResult {
        const NAME: &str = "native::owned::schema::tests::native_schema_replay_revalidates_durable_copies_after_process_death";
        const CONTEXT: &str = "LIBRAIRII_SCHEMA_RECOVERY_CRASH";
        if let Some(raw) = std::env::var_os(CONTEXT) {
            let context: serde_json::Value = serde_json::from_slice(raw.as_encoded_bytes())?;
            let locations = ProfileLocations {
                native: PathBuf::from(context["native"].as_str().ok_or("native context")?),
                data: PathBuf::from(context["data"].as_str().ok_or("data context")?),
                legacy: context["legacy"].as_str().map(PathBuf::from),
            };
            let store = NativeProfileStore::open_native(&locations.native)?;
            let recovery = SchemaRecovery::open(&store, &locations, &Default::default())?;
            let _result = recovery.confirm(unclosed);
            return Err("child did not hold its requested actual schema boundary".into());
        }
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _killed = self.0.kill();
                let _joined = self.0.wait();
            }
        }
        for point in 0..3 {
            let (roots, locations, root) = setup()?;
            let before = payloads(&root.join("archives"))?;
            let directory = roots.profile().join("schema-crash");
            fs::create_dir(&directory)?;
            fs::write(
                directory.join(".schema-recovery-fixture"),
                b"schema-recovery-v1\n",
            )?;
            let ready = directory.join("ready");
            let context = serde_json::json!({"native":locations.native,"data":locations.data,"legacy":locations.legacy,"point":point,"ready":ready});
            let output = fs::File::create(directory.join("execution.log"))?;
            let mut child = Child(
                std::process::Command::new(std::env::current_exe()?)
                    .args(["--exact", NAME, "--nocapture"])
                    .env(CONTEXT, serde_json::to_string(&context)?)
                    .stdout(std::process::Stdio::from(output.try_clone()?))
                    .stderr(std::process::Stdio::from(output))
                    .spawn()?,
            );
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !ready.exists() {
                if child.0.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
                    return Err(format!(
                        "schema child failed before checkpoint: {}",
                        String::from_utf8_lossy(&fs::read(directory.join("execution.log"))?)
                    )
                    .into());
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            child.0.kill()?;
            child.0.wait()?;
            let store = NativeProfileStore::open_native(&locations.native)?;
            let recovery = if point == 2 {
                let interrupted =
                    NativeLibrary::recover_interrupted(&store, &locations, &Default::default())?;
                assert!(interrupted.has_pending_schema_recovery()?);
                interrupted.into_schema_recovery()
            } else {
                SchemaRecovery::open(&store, &locations, &Default::default())?
            };
            assert_eq!(
                recovery.preview(),
                Some(
                    [
                        SchemaPreview::Foreign,
                        SchemaPreview::ResumePreserved,
                        SchemaPreview::FinishReplacement
                    ][point]
                )
            );
            let (native, paths) = recovery.confirm(unclosed).map_err(|f| f.to_string())?;
            assert_eq!(paths.len(), 1);
            verify_preserved(&paths[0])?;
            assert_eq!(payloads(&root.join("archives"))?, before);
            native.finish().map_err(|f| f.to_string())?;
            let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
            let mut db = crate::database::Database::open_native(&native)?;
            db.close()?;
            drop(db);
            native.finish().map_err(|f| f.to_string())?;
        }
        Ok(())
    }
}
