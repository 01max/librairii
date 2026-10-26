//! Independent version-1 profile data. Reading a profile never grants library authority.
//!
//! Legacy settings are read-only inputs. Native updates require a fresh, separate
//! namespace with its marker locked for the entire store lifetime. This lock does
//! not exclude an unmodified Go library process or authorize library adoption.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod navigation;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) mod relocation;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::platform::unix::{ConfigTemp, ControlAllocation, Directory, durable_file_flush};
use crate::{Result, StorageError, backup};
use serde::{Deserialize, Serialize, de};
use std::{
    fmt,
    fs::{self, File, Metadata},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use std::fs::OpenOptions;

pub const MAX_PROFILE_BYTES: usize = 64 * 1024;
const SETTINGS: &str = "settings.json";
pub(crate) const MARKER: &str = ".librairii-native-profile";
pub(crate) const MARKER_BYTES: &[u8] = b"librairii-native-profile-v1\n";

/// Wire data, not a library lease, identity proof, or recovery permission.
/// `library_id` retains the exact legacy spelling; it is deliberately not UUID
/// Display serialization. Incomplete setup may retain unbound root/ID data.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Profile {
    pub version: i64,
    pub language: String,
    pub root: String,
    #[serde(rename = "libraryId")]
    pub library_id: String,
    pub complete: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub recovering: bool,
}

impl Profile {
    pub fn setup() -> Self {
        Self {
            version: 1,
            language: "en".into(),
            ..Self::default()
        }
    }

    /// Native-host path validation follows Go Profile.Validate. Cross-OS
    /// authority must be independently rebound before activation (task4.10).
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || !matches!(self.language.as_str(), "en" | "fr")
            || ((self.complete || self.recovering)
                && (!Path::new(&self.root).is_absolute() || self.library_id.is_empty()))
        {
            return Err(StorageError::InvalidProfile);
        }
        Ok(())
    }
}

// Go's decoder matches field names case-insensitively, keeps the last non-null
// scalar assignment, and leaves scalars unchanged for null. Preserve that wire
// behavior while rejecting unknown fields and all non-object top-level values.
impl<'de> Deserialize<'de> for Profile {
    fn deserialize<D: de::Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct ProfileVisitor;
        impl<'de> de::Visitor<'de> for ProfileVisitor {
            type Value = Profile;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a version-1 settings object")
            }
            fn visit_map<A: de::MapAccess<'de>>(
                self,
                mut fields: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut profile = Profile::default();
                while let Some(key) = fields.next_key::<String>()? {
                    // Go's simple case fold maps long-s to S too (version).
                    let key = key.replace('\u{17f}', "s").to_ascii_lowercase();
                    match key.as_str() {
                        "version" => {
                            if let Some(v) = fields.next_value::<Option<i64>>()? {
                                profile.version = v;
                            }
                        }
                        "language" => {
                            if let Some(v) = fields.next_value::<Option<String>>()? {
                                profile.language = v;
                            }
                        }
                        "root" => {
                            if let Some(v) = fields.next_value::<Option<String>>()? {
                                profile.root = v;
                            }
                        }
                        "libraryid" => {
                            if let Some(v) = fields.next_value::<Option<String>>()? {
                                profile.library_id = v;
                            }
                        }
                        "complete" => {
                            if let Some(v) = fields.next_value::<Option<bool>>()? {
                                profile.complete = v;
                            }
                        }
                        "recovering" => {
                            if let Some(v) = fields.next_value::<Option<bool>>()? {
                                profile.recovering = v;
                            }
                        }
                        _ => return Err(de::Error::custom("unknown settings field")),
                    }
                }
                Ok(profile)
            }
        }
        decoder.deserialize_map(ProfileVisitor)
    }
}

/// Exact source bytes remain available; decoding alone never rewrites them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileDocument {
    profile: Profile,
    original: Vec<u8>,
}

impl ProfileDocument {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_PROFILE_BYTES {
            return Err(StorageError::InvalidProfile);
        }
        let profile: Profile =
            serde_json::from_slice(bytes).map_err(|_| StorageError::InvalidProfile)?;
        profile.validate()?;
        Ok(Self {
            profile,
            original: bytes.to_vec(),
        })
    }
    pub fn profile(&self) -> &Profile {
        &self.profile
    }
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }
}

/// Inspect an explicitly supplied legacy directory without creation, repair,
/// caching, fallback to defaults, or reading library/recovery paths from it.
pub fn read_legacy(directory: &Path) -> Result<Option<ProfileDocument>> {
    read_document(&directory.join(SETTINGS))
}

fn read_document(path: &Path) -> Result<Option<ProfileDocument>> {
    Ok(read_document_with_witness(path)?.map(|(document, _)| document))
}

fn read_document_with_witness(path: &Path) -> Result<Option<(ProfileDocument, Metadata)>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(StorageError::io("inspect settings", error)),
    };
    require_regular(&metadata)?;
    let file = File::open(path).map_err(|e| StorageError::io("read settings", e))?;
    require_same(
        &metadata,
        &file
            .metadata()
            .map_err(|e| StorageError::io("inspect open settings", e))?,
    )?;
    decode_open_document(file, metadata).map(Some)
}

fn decode_open_document(file: File, metadata: Metadata) -> Result<(ProfileDocument, Metadata)> {
    let mut bytes = Vec::new();
    file.take((MAX_PROFILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| StorageError::io("read bounded settings", e))?;
    Ok((ProfileDocument::decode(&bytes)?, metadata))
}

#[derive(Debug, Default)]
struct State {
    loaded: bool,
    document: Option<ProfileDocument>,
    settings_metadata: Option<Metadata>,
    poisoned: bool,
    binding: Option<Arc<BindingState>>,
    // Actual open-object witnesses survive a post-replace error through the
    // poisoned store's lifetime. They are never purged by Drop/restart scanning.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    recovery_witnesses: Vec<ConfigTemp>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    recovery_allocations: Vec<ControlAllocation>,
}

impl State {
    fn poison(&mut self) {
        self.poisoned = true;
        if let Some(binding) = &self.binding {
            binding.fenced.store(true, Ordering::Release);
        }
    }
}

/// Storage-private exclusion, not a public profile-data-to-authority cast.
/// Its Arc identity distinguishes successive bindings without serialized IDs.
#[derive(Debug)]
struct BindingState {
    root: String,
    raw_id: String,
    fenced: Arc<AtomicBool>,
    frozen: AtomicBool,
}
impl BindingState {
    fn matches(&self, profile: &Profile) -> bool {
        profile.root == self.root
            && profile.library_id == self.raw_id
            && profile.complete
            && !profile.recovering
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[derive(Debug)]
pub(crate) struct ProfileBinding<'profile> {
    store: &'profile NativeProfileStore,
    state: Arc<BindingState>,
    released: bool,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl ProfileBinding<'_> {
    pub(crate) fn fence(&self) {
        self.state.fenced.store(true, Ordering::Release);
    }
    pub(crate) fn fence_signal(&self) -> Arc<AtomicBool> {
        self.state.fenced.clone()
    }
    pub(crate) fn revalidate(&self) -> Result<()> {
        if self.state.fenced.load(Ordering::Acquire) || self.state.frozen.load(Ordering::Acquire) {
            return Err(StorageError::ProfileRestartRequired);
        }
        let mut state = self.store.reserve_inner(true)?;
        let result = (|| {
            self.store.load_locked(&mut state)?;
            self.store.revalidate_settings(&state)?;
            self.store.require_unrecovered_namespace()?;
            if self.state.fenced.load(Ordering::Acquire)
                || !state
                    .binding
                    .as_ref()
                    .is_some_and(|active| Arc::ptr_eq(active, &self.state))
                || !state
                    .document
                    .as_ref()
                    .is_some_and(|document| self.state.matches(document.profile()))
            {
                return Err(StorageError::ProfileRestartRequired);
            }
            Ok(())
        })();
        if result.is_err() {
            state.poison();
        }
        result
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Drop for ProfileBinding<'_> {
    fn drop(&mut self) {
        // Dropping an owner is never clean-closure proof, even after DB close.
        if !self.released {
            self.fence();
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[derive(Debug)]
pub(crate) struct FrozenProfile<'profile> {
    reservation: StateReservation<'profile>,
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
impl<'profile> ProfileBinding<'profile> {
    pub(crate) fn freeze(&self) -> Result<FrozenProfile<'profile>> {
        let reservation = self.store.reserve_inner(true)?;
        if !reservation
            .binding
            .as_ref()
            .is_some_and(|b| Arc::ptr_eq(b, &self.state))
        {
            return Err(StorageError::ProfileRestartRequired);
        }
        self.state.frozen.store(true, Ordering::Release);
        Ok(FrozenProfile { reservation })
    }
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
impl FrozenProfile<'_> {
    pub(crate) fn thaw(self) {
        // A positively closed library still retains its binding and OS lease.
        // Only release the short publication reservation so independent
        // language updates remain possible during the long relocation copy.
        if let Some(binding) = &self.reservation.binding {
            binding.frozen.store(false, Ordering::Release);
        }
    }
    pub(crate) fn revalidate(&self) -> Result<()> {
        let store = self.reservation.store;
        if self.reservation.poisoned {
            return Err(StorageError::ProfileRestartRequired);
        }
        let binding = self
            .reservation
            .binding
            .as_ref()
            .ok_or(StorageError::ProfileRestartRequired)?;
        if binding.fenced.load(Ordering::Acquire)
            || !self
                .reservation
                .document
                .as_ref()
                .is_some_and(|d| binding.matches(d.profile()))
        {
            return Err(StorageError::ProfileRestartRequired);
        }
        store.revalidate_settings(&self.reservation)?;
        store.require_unrecovered_namespace()
    }
    pub(crate) fn release(mut self, mut binding: ProfileBinding<'_>) {
        // Only the consuming NativeLibrary finalizer calls this AFTER all root
        // descriptors and the root lease close. This owned reservation excludes
        // every admitted profile update through that ordering point.
        self.reservation.binding = None;
        binding.released = true;
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) struct BindingReservation<'profile> {
    reservation: StateReservation<'profile>,
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
impl<'profile> BindingReservation<'profile> {
    pub(crate) fn publish(mut self, proposed: &Profile) -> Result<ProfileBinding<'profile>> {
        self.reservation
            .store
            .publish_binding(&mut self.reservation.state, proposed)
    }
}

/// Serialized updates in a separately initialized native namespace. The marker
/// recognizes the namespace format, not trust in a copied library or journal.
///
/// Native Unix effects are relative to the retained namespace directory. They
/// require the marker OS lock and state mutex; replacement preserves displaced
/// config bytes until their exact prior identity/content has been verified.
/// This grants no production library adoption authority; Windows remains4.9.
#[derive(Debug)]
pub struct NativeProfileStore {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    move_journal: Mutex<Option<relocation::JournalWitness>>,
    directory: PathBuf,
    directory_metadata: Metadata,
    marker: File,
    marker_metadata: Metadata,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    namespace: Directory,
    // None means one bounded synchronous operation owns the actual state.
    // The mutex protects admission/transfer only, never callbacks or I/O.
    state: Mutex<Option<State>>,
}

#[derive(Debug)]
struct StateReservation<'store> {
    store: &'store NativeProfileStore,
    state: State,
}
impl std::ops::Deref for StateReservation<'_> {
    type Target = State;
    fn deref(&self) -> &State {
        &self.state
    }
}
impl std::ops::DerefMut for StateReservation<'_> {
    fn deref_mut(&mut self) -> &mut State {
        &mut self.state
    }
}
impl Drop for StateReservation<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.state.poison();
        }
        let mut slot = match self.store.state.lock() {
            Ok(slot) => slot,
            Err(poison) => {
                self.state.poison();
                poison.into_inner()
            }
        };
        *slot = Some(std::mem::take(&mut self.state));
    }
}

impl NativeProfileStore {
    fn reserve(&self) -> Result<StateReservation<'_>> {
        self.reserve_inner(false)
    }
    fn reserve_inner(&self, frozen_allowed: bool) -> Result<StateReservation<'_>> {
        let mut slot = self
            .state
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)?;
        let state = slot.take().ok_or(StorageError::ProfileBusy)?;
        if !frozen_allowed
            && (state.poisoned
                || state
                    .binding
                    .as_ref()
                    .is_some_and(|b| b.fenced.load(Ordering::Acquire)))
        {
            *slot = Some(state);
            return Err(StorageError::ProfileRestartRequired);
        }
        if !frozen_allowed
            && state
                .binding
                .as_ref()
                .is_some_and(|b| b.frozen.load(Ordering::Acquire))
        {
            *slot = Some(state);
            return Err(StorageError::ProfileBusy);
        }
        Ok(StateReservation { store: self, state })
    }
    /// Initialize only a fresh explicit directory, never a legacy settings
    /// directory. Failure leaves any created namespace bytes for inspection.
    pub fn create_fresh(directory: &Path) -> Result<Self> {
        backup::require_directory_durability()?;
        if !directory.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        let initial_directory = match fs::symlink_metadata(directory) {
            Ok(metadata) => {
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || fs::read_dir(directory)
                        .map_err(|e| StorageError::io("inspect fresh profile namespace", e))?
                        .next()
                        .is_some()
                {
                    return Err(StorageError::RootNotFresh);
                }
                metadata
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let builder = fs::DirBuilder::new();
                #[cfg(unix)]
                let mut builder = builder;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder
                    .create(directory)
                    .map_err(|e| StorageError::io("create native profile namespace", e))?;
                fs::symlink_metadata(directory)
                    .map_err(|e| StorageError::io("retain fresh profile directory", e))?
            }
            Err(error) => return Err(StorageError::io("inspect profile namespace", error)),
        };
        if !initial_directory.is_dir() || initial_directory.file_type().is_symlink() {
            return Err(StorageError::RootNotFresh);
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let namespace = Directory::open(directory)?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        namespace.matches_metadata(&initial_directory)?;
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let mut options = OpenOptions::new();
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        options.write(true).create_new(true);
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let mut marker = options
            .open(directory.join(MARKER))
            .map_err(|e| StorageError::io("create native profile marker", e))?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let mut marker = namespace.open_regular(
            std::ffi::OsStr::new(MARKER),
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CREATE | rustix::fs::OFlags::EXCL,
        )?;
        marker
            .write_all(MARKER_BYTES)
            .and_then(|()| marker.sync_all())
            .map_err(|e| StorageError::io("flush native profile marker", e))?;
        backup::sync_directory(directory)?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            durable_file_flush(&marker)?;
            namespace.flush()?;
        }
        if let Some(parent) = directory.parent() {
            backup::sync_directory(parent)?;
        }
        Self::open_native(directory)
    }

    /// Reopen only a previously initialized native namespace. Neither this
    /// marker nor its lock grants authority to any profile-referenced library.
    pub fn open_native(directory: &Path) -> Result<Self> {
        backup::require_directory_durability()?;
        if !directory.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        let directory_metadata = fs::symlink_metadata(directory)
            .map_err(|e| StorageError::io("inspect native namespace", e))?;
        if !directory_metadata.is_dir() || directory_metadata.file_type().is_symlink() {
            return Err(StorageError::PathEscape);
        }
        let path = directory.join(MARKER);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| StorageError::io("inspect native profile marker", e))?;
        require_regular(&metadata)?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let namespace = Directory::open(directory)?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        namespace.matches_metadata(&directory_metadata)?;
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let mut marker = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| StorageError::io("open native profile marker", e))?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let mut marker =
            namespace.open_regular(std::ffi::OsStr::new(MARKER), rustix::fs::OFlags::RDWR)?;
        let marker_metadata = marker
            .metadata()
            .map_err(|e| StorageError::io("inspect open profile marker", e))?;
        require_same(&metadata, &marker_metadata)?;
        marker.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => StorageError::ProfileInUse,
            fs::TryLockError::Error(error) => StorageError::io("lock native profile", error),
        })?;
        let mut bytes = Vec::new();
        (&mut marker)
            .take((MARKER_BYTES.len() + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read native profile marker", e))?;
        if bytes != MARKER_BYTES {
            return Err(StorageError::InvalidProfile);
        }
        let store = Self {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            move_journal: Mutex::new(None),
            directory: directory.to_path_buf(),
            directory_metadata,
            marker,
            marker_metadata,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            namespace,
            state: Mutex::new(Some(State::default())),
        };
        store.revalidate_namespace()?;
        Ok(store)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn marker_digest(&self) -> Result<String> {
        use sha2::Digest;
        self.revalidate_namespace()?;
        Ok(
            librairii_core::Checksum::from_bytes(sha2::Sha256::digest(MARKER_BYTES).into())
                .to_string(),
        )
    }
    pub fn recovery_directory(&self) -> PathBuf {
        self.directory.join("recovery")
    }

    pub fn load(&self) -> Result<Option<ProfileDocument>> {
        let mut state = self.reserve()?;
        self.load_locked(&mut state)?;
        Ok(state.document.clone())
    }

    /// Change language or retain the current authority fields. Root/raw-ID/
    /// complete/recovering publication is reserved to the actual owned native
    /// binding route, including when no binding is active. The callback must
    /// be bounded and synchronous; a racing owner receives ProfileBusy.
    pub fn update(&self, change: impl FnOnce(&mut Profile) -> Result<()>) -> Result<Profile> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        return self.update_with_flush(change, |_| Ok(()));
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        self.update_with_flush(change, backup::sync_directory)
    }

    fn load_locked(&self, state: &mut State) -> Result<()> {
        if state.poisoned {
            return Err(StorageError::ProfileRestartRequired);
        }
        if let Err(error) = self.revalidate_namespace() {
            if state.binding.is_some() {
                state.poison();
            }
            return Err(error);
        }
        if !state.loaded {
            if let Some((document, metadata)) = self.read_native_document()? {
                state.document = Some(document);
                state.settings_metadata = Some(metadata);
            }
            state.loaded = true;
        }
        Ok(())
    }

    /// Only native.rs may invoke this after checking an owned independent copy
    /// and the explicit native namespace. No filesystem/database copy or caller
    /// callback is run with this short profile-publication mutex held.
    #[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
    pub(crate) fn bind_verified_copy(&self, proposed: &Profile) -> Result<ProfileBinding<'_>> {
        self.begin_binding()?.publish(proposed)
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn begin_binding(&self) -> Result<BindingReservation<'_>> {
        let mut state = self.reserve()?;
        self.load_locked(&mut state)?;
        self.revalidate_settings(&state)?;
        if state.binding.is_some() {
            return Err(StorageError::ProfileBindingActive);
        }
        self.require_unrecovered_namespace()?;
        if state
            .document
            .as_ref()
            .is_some_and(|d| d.profile().recovering)
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(BindingReservation { reservation: state })
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn publish_binding<'profile>(
        &'profile self,
        state: &mut State,
        proposed: &Profile,
    ) -> Result<ProfileBinding<'profile>> {
        let mut next = proposed.clone();
        if let Some(document) = &state.document {
            let current = document.profile();
            if current.recovering {
                return Err(StorageError::RecoveryBindingRequired);
            }
            let unbound =
                !current.complete && current.root.is_empty() && current.library_id.is_empty();
            let matching = current.complete
                && current.root == proposed.root
                && current.library_id == proposed.library_id;
            if !unbound && !matching {
                return Err(StorageError::ProfileChanged);
            }
            // An already chosen native language is newer independent authority.
            next.language.clone_from(&current.language);
        }
        next.validate()?;
        if !next.complete || next.recovering {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let bytes = encode(&next)?;
        if state.document.as_ref().map(ProfileDocument::profile) != Some(&next) {
            self.update_unix(state, next.clone(), bytes, |_| Ok(()), |_| Ok(()))?;
        }
        let binding = Arc::new(BindingState {
            root: next.root,
            raw_id: next.library_id,
            fenced: Arc::new(AtomicBool::new(false)),
            frozen: AtomicBool::new(false),
        });
        state.binding = Some(binding.clone());
        Ok(ProfileBinding {
            store: self,
            state: binding,
            released: false,
        })
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn require_unrecovered_namespace(&self) -> Result<()> {
        self.revalidate_namespace()?;
        let entries = self
            .namespace
            .entries(4, &crate::database::WorkCancellation::default())
            .map_err(|error| match error {
                StorageError::AdoptionInputChanged => StorageError::RecoveryBindingRequired,
                error => error,
            })?;
        if entries.iter().any(|entry| {
            entry != MARKER
                && entry != SETTINGS
                && entry != "navigation.json"
                && entry != relocation::JOURNAL
        }) {
            return Err(StorageError::RecoveryBindingRequired);
        }
        if entries.iter().any(|entry| entry == relocation::JOURNAL) {
            self.revalidate_move_journal()?;
        }
        if entries.iter().any(|entry| entry == "navigation.json") {
            // Independent UI preferences grant no binding/recovery authority.
            // Refuse links/special files; malformed bytes are reported only by
            // the UI preference reader and never authorize cleanup or replay.
            self.namespace.open_regular(
                std::ffi::OsStr::new("navigation.json"),
                rustix::fs::OFlags::RDONLY,
            )?;
        }
        Ok(())
    }

    fn update_with_flush(
        &self,
        change: impl FnOnce(&mut Profile) -> Result<()>,
        flush: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<Profile> {
        self.update_with_checkpoints(change, |_| Ok(()), flush)
    }

    // Private synchronous fault seams at real durability boundaries. Public
    // callers cannot provide filesystem work or a replacement publisher here.
    fn update_with_checkpoints(
        &self,
        change: impl FnOnce(&mut Profile) -> Result<()>,
        before_replace: impl FnOnce(&Path) -> Result<()>,
        flush: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<Profile> {
        let mut state = self.reserve()?;
        self.load_locked(&mut state)?;
        if let Err(error) = self.revalidate_settings(&state) {
            if state.binding.is_some() {
                state.poison();
            }
            return Err(error);
        }
        let mut next = state
            .document
            .as_ref()
            .map_or_else(Profile::setup, |d| d.profile.clone());
        change(&mut next)?;
        next.validate()?;
        if let Some(binding) = &state.binding {
            if binding.fenced.load(Ordering::Acquire) {
                return Err(StorageError::ProfileRestartRequired);
            }
            if binding.frozen.load(Ordering::Acquire) {
                return Err(StorageError::ProfileBusy);
            }
            if !binding.matches(&next) {
                return Err(StorageError::ProfileBindingActive);
            }
        }
        let previous = state
            .document
            .as_ref()
            .map_or_else(Profile::setup, |d| d.profile.clone());
        let bytes = encode(&next)?;
        if (
            next.root.as_str(),
            next.library_id.as_str(),
            next.complete,
            next.recovering,
        ) != (
            previous.root.as_str(),
            previous.library_id.as_str(),
            previous.complete,
            previous.recovering,
        ) {
            return Err(StorageError::ProfileBindingActive);
        }
        // The profile namespace/OS lease owns independent language settings
        // even when an interrupted library cannot be rebound. Authority fields
        // are unchanged above; this never clears the library's persisted Active
        // state or grants runtime/recovery permission.
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        return self.update_unix(&mut state, next, bytes, before_replace, flush);
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let mut staged = tempfile::Builder::new()
                .prefix(".settings-")
                .tempfile_in(&self.directory)
                .map_err(|e| StorageError::io("stage native settings", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                staged
                    .as_file()
                    .set_permissions(fs::Permissions::from_mode(0o600))
                    .map_err(|e| StorageError::io("protect staged settings", e))?;
            }
            staged
                .write_all(&bytes)
                .and_then(|()| staged.as_file().sync_all())
                .map_err(|e| StorageError::io("flush staged settings", e))?;
            let new_metadata = staged
                .as_file()
                .metadata()
                .map_err(|e| StorageError::io("inspect staged settings", e))?;
            before_replace(staged.path())?;
            self.revalidate_namespace()?;
            self.revalidate_settings(&state)?;
            // This is replace-owned-config, never a no-clobber archive operation.
            staged
                .persist(self.directory.join(SETTINGS))
                .map_err(|e| StorageError::io("replace native settings", e.error))?;
            let durability = (|| {
                flush(&self.directory)?;
                self.revalidate_namespace()?;
                let published = fs::symlink_metadata(self.directory.join(SETTINGS))
                    .map_err(|e| StorageError::io("inspect published settings", e))?;
                require_regular(&published)?;
                require_same(&new_metadata, &published)
            })();
            if let Err(cause) = durability {
                state.poison();
                return Err(StorageError::ProfilePublicationUncertain {
                    cause: Box::new(cause),
                });
            }
            state.document = Some(ProfileDocument {
                profile: next.clone(),
                original: bytes,
            });
            state.settings_metadata = Some(new_metadata);
            Ok(next)
        }
    }

    fn revalidate_settings(&self, state: &State) -> Result<()> {
        match (self.read_native_document()?, &state.settings_metadata) {
            (None, None) if state.document.is_none() => Ok(()),
            (Some((document, metadata)), Some(previous))
                if Some(&document) == state.document.as_ref() =>
            {
                require_same(previous, &metadata)
            }
            _ => Err(StorageError::ProfileChanged),
        }
    }

    fn revalidate_namespace(&self) -> Result<()> {
        let directory = fs::symlink_metadata(&self.directory)
            .map_err(|e| StorageError::io("revalidate profile directory", e))?;
        if directory.file_type().is_symlink() || !directory.is_dir() {
            return Err(StorageError::PathEscape);
        }
        require_same(&self.directory_metadata, &directory)?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        self.namespace.matches_metadata(&directory)?;
        let marker = fs::symlink_metadata(self.directory.join(MARKER))
            .map_err(|e| StorageError::io("revalidate profile marker", e))?;
        require_regular(&marker)?;
        require_same(&self.marker_metadata, &marker)?;
        let mut bytes = Vec::new();
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let opened = File::open(self.directory.join(MARKER))
            .map_err(|e| StorageError::io("reopen profile marker witness", e))?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let opened = self
            .namespace
            .open_regular(std::ffi::OsStr::new(MARKER), rustix::fs::OFlags::RDONLY)?;
        require_same(
            &self.marker_metadata,
            &opened
                .metadata()
                .map_err(|e| StorageError::io("inspect anchored marker", e))?,
        )?;
        opened
            .take((MARKER_BYTES.len() + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("recheck profile marker bytes", e))?;
        if bytes != MARKER_BYTES {
            return Err(StorageError::ProfileChanged);
        }
        require_same(
            &self.marker_metadata,
            &self
                .marker
                .metadata()
                .map_err(|e| StorageError::io("revalidate retained marker", e))?,
        )
    }

    fn read_native_document(&self) -> Result<Option<(ProfileDocument, Metadata)>> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let Some(file) = self
                .namespace
                .read_regular(std::ffi::OsStr::new(SETTINGS))?
            else {
                return Ok(None);
            };
            let metadata = file
                .metadata()
                .map_err(|e| StorageError::io("inspect anchored settings", e))?;
            decode_open_document(file, metadata).map(Some)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        read_document_with_witness(&self.directory.join(SETTINGS))
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn update_unix(
        &self,
        state: &mut State,
        next: Profile,
        bytes: Vec<u8>,
        before_replace: impl FnOnce(&Path) -> Result<()>,
        flush: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<Profile> {
        let mut staged = match self.namespace.create_config_temp() {
            Ok(staged) => staged,
            Err(failure) => {
                let failure = *failure;
                state.poison();
                state.recovery_allocations.push(failure.allocation);
                return Err(failure.cause);
            }
        };
        let before = (|| {
            staged
                .file
                .write_all(&bytes)
                .map_err(|e| StorageError::io("write anchored settings stage", e))?;
            durable_file_flush(&staged.file)?;
            before_replace(&self.directory.join(&staged.name))?;
            self.revalidate_namespace()?;
            self.revalidate_settings(state)
        })();
        if let Err(work) = before {
            return match self.namespace.remove_config_temp(&staged) {
                Ok(()) => Err(work),
                Err(cleanup) => {
                    state.poison();
                    state.recovery_witnesses.push(staged);
                    Err(StorageError::OwnedTempCleanupFailed {
                        work: Box::new(work),
                        cleanup: Box::new(cleanup),
                    })
                }
            };
        }
        let (published, old) = match self.namespace.replace_owned_config(
            staged,
            std::ffi::OsStr::new(SETTINGS),
            state.settings_metadata.as_ref(),
        ) {
            Ok(receipt) => receipt,
            Err(failure) => {
                let failure = *failure;
                if failure.changed {
                    state.poison();
                    state.recovery_witnesses.push(failure.temporary);
                    return Err(StorageError::ProfilePublicationUncertain {
                        cause: Box::new(failure.cause),
                    });
                }
                return match self.namespace.remove_config_temp(&failure.temporary) {
                    Ok(()) => Err(failure.cause),
                    Err(cleanup) => {
                        state.poison();
                        state.recovery_witnesses.push(failure.temporary);
                        Err(StorageError::OwnedTempCleanupFailed {
                            work: Box::new(failure.cause),
                            cleanup: Box::new(cleanup),
                        })
                    }
                };
            }
        };
        let after = (|| {
            // EXCHANGE preserved the prior object. Verify BOTH identity and raw
            // bytes before removing its now-private temporary name.
            if let Some(old) = &old {
                let mut file = old
                    .file
                    .try_clone()
                    .map_err(|e| StorageError::io("retain displaced config", e))?;
                let mut prior = Vec::new();
                (&mut file)
                    .take((MAX_PROFILE_BYTES + 1) as u64)
                    .read_to_end(&mut prior)
                    .map_err(|e| StorageError::io("read displaced config", e))?;
                if state.document.as_ref().map(ProfileDocument::original_bytes)
                    != Some(prior.as_slice())
                {
                    return Err(StorageError::ProfileChanged);
                }
            }
            self.namespace.flush()?;
            flush(&self.directory)?;
            self.revalidate_namespace()?;
            let (actual, metadata) = self
                .read_native_document()?
                .ok_or(StorageError::ProfileChanged)?;
            if actual.original_bytes() != bytes {
                return Err(StorageError::ProfileChanged);
            }
            require_same(
                &published
                    .file
                    .metadata()
                    .map_err(|e| StorageError::io("inspect published config witness", e))?,
                &metadata,
            )?;
            if let Some(old) = &old {
                self.namespace.remove_config_temp(old)?;
                self.namespace.flush()?;
            }
            Ok(metadata)
        })();
        match after {
            Ok(metadata) => {
                state.document = Some(ProfileDocument {
                    profile: next.clone(),
                    original: bytes,
                });
                state.settings_metadata = Some(metadata);
                Ok(next)
            }
            Err(cause) => {
                state.poison();
                state.recovery_witnesses.push(published);
                state.recovery_witnesses.extend(old);
                Err(StorageError::ProfilePublicationUncertain {
                    cause: Box::new(cause),
                })
            }
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn encoded_document(profile: &Profile) -> Result<ProfileDocument> {
    ProfileDocument::decode(&encode(profile)?)
}

fn encode(profile: &Profile) -> Result<Vec<u8>> {
    let json = serde_json::to_string_pretty(profile).map_err(|_| StorageError::InvalidProfile)?;
    // Match encoding/json's default HTML/JavaScript-safe string spelling.
    let json = json
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    if json.len() > MAX_PROFILE_BYTES {
        return Err(StorageError::InvalidProfile);
    }
    Ok(json.into_bytes())
}

fn require_regular(metadata: &Metadata) -> Result<()> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StorageError::PathEscape);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(StorageError::PathEscape);
        }
    }
    Ok(())
}

fn require_same(before: &Metadata, after: &Metadata) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() == after.dev() && before.ino() == after.ino() {
            return Ok(());
        }
    }
    #[cfg(not(unix))]
    {
        // Read-only decoding is permitted; native mutation constructors refuse
        // before effects until task4.9 supplies Windows identity/durability.
        if before.len() == after.len() && before.modified().ok() == after.modified().ok() {
            return Ok(());
        }
    }
    Err(StorageError::ProfileChanged)
}

/// Pure native-host location selection from explicit inputs. No environment,
/// home lookup, directory creation, profile discovery or authority is implied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileLocations {
    pub legacy: Option<PathBuf>,
    pub native: PathBuf,
    pub data: PathBuf,
}

impl ProfileLocations {
    pub fn for_host(
        home: &Path,
        config_base: Option<&Path>,
        data_base: Option<&Path>,
    ) -> Result<Self> {
        if !home.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        #[cfg(target_os = "macos")]
        {
            let _ = (config_base, data_base);
            Ok(Self {
                legacy: Some(home.join("Library/Preferences/io.github.librairii.app")),
                native: home.join("Library/Preferences/io.github.librairii.native"),
                data: home.join("Library/Application Support/LibrairiiNative"),
            })
        }
        #[cfg(target_os = "linux")]
        {
            let config = config_base.map_or_else(|| home.join(".config"), Path::to_path_buf);
            let data = data_base.map_or_else(|| home.join(".local/share"), Path::to_path_buf);
            if !config.is_absolute() || !data.is_absolute() {
                return Err(StorageError::RootNotAbsolute);
            }
            Ok(Self {
                legacy: Some(config.join("librairii")),
                native: config.join("librairii-native"),
                data: data.join("librairii-native"),
            })
        }
        #[cfg(target_os = "windows")]
        {
            // Caller supplies APPDATA and LOCALAPPDATA; never guess from home.
            let config = config_base.ok_or(StorageError::InvalidProfile)?;
            let data = data_base.ok_or(StorageError::InvalidProfile)?;
            if !config.is_absolute() || !data.is_absolute() {
                return Err(StorageError::RootNotAbsolute);
            }
            Ok(Self {
                legacy: None,
                native: config.join("LibrairiiNative"),
                data: data.join("LibrairiiNative").join("Data"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use librairii_testkit::TestRoots;
    use std::error::Error;

    #[test]
    #[cfg(unix)]
    fn post_replace_flush_failure_poison_blocks_cache_and_updates_until_reopen()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let directory = roots.profile().join("native");
        let store = NativeProfileStore::create_fresh(&directory)?;
        store.update(|_| Ok(()))?;
        let result = store.update_with_flush(
            |p| {
                p.language = "fr".into();
                Ok(())
            },
            |_| {
                Err(StorageError::io(
                    "injected directory flush",
                    std::io::Error::other("fault after replacement"),
                ))
            },
        );
        assert!(matches!(
            result,
            Err(StorageError::ProfilePublicationUncertain { .. })
        ));
        assert!(matches!(
            store.load(),
            Err(StorageError::ProfileRestartRequired)
        ));
        assert!(matches!(
            store.update(|p| {
                p.language = "en".into();
                Ok(())
            }),
            Err(StorageError::ProfileRestartRequired)
        ));
        let durable_observation = read_legacy(&directory)?.ok_or("missing replaced bytes")?;
        assert_eq!(durable_observation.profile().language, "fr");
        drop(store);
        assert_eq!(
            NativeProfileStore::open_native(&directory)?
                .load()?
                .ok_or("missing restart bytes")?,
            durable_observation
        );
        Ok(())
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn active_binding_post_replace_uncertainty_keeps_exclusion_after_guard_drop()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let directory = roots.profile().join("native");
        let store = NativeProfileStore::create_fresh(&directory)?;
        // Private binding mechanism test; this is not a runtime-authority or
        // verified-copy constructor. The native integration tests use the real
        // owned-copy route instead of this private profile-only fixture.
        let proposed = Profile {
            version: 1,
            language: "en".into(),
            root: roots.library().to_str().ok_or("root UTF8")?.into(),
            library_id: "A0000000-0000-4000-8000-000000000001".into(),
            complete: true,
            recovering: false,
        };
        let binding = store.bind_verified_copy(&proposed)?;
        let result = store.update_with_flush(
            |profile| {
                profile.language = "fr".into();
                Ok(())
            },
            |_| {
                Err(StorageError::io(
                    "injected bound profile directory sync",
                    std::io::Error::other("fault after real replace"),
                ))
            },
        );
        assert!(matches!(
            result,
            Err(StorageError::ProfilePublicationUncertain { .. })
        ));
        assert!(matches!(
            binding.revalidate(),
            Err(StorageError::ProfileRestartRequired)
        ));
        drop(binding);
        let slot = store.state.lock().map_err(|_| "test state poisoned")?;
        let state = slot.as_ref().ok_or("busy state")?;
        assert!(state.binding.is_some());
        assert!(state.poisoned);
        drop(slot);
        assert!(matches!(
            store.update(|_| Ok(())),
            Err(StorageError::ProfileRestartRequired)
        ));
        assert!(matches!(
            store.bind_verified_copy(&proposed),
            Err(StorageError::ProfileRestartRequired)
        ));
        assert_eq!(
            read_legacy(&directory)?
                .ok_or("published settings")?
                .profile()
                .language,
            "fr"
        );
        drop(store);
        assert_eq!(
            NativeProfileStore::open_native(&directory)?
                .load()?
                .ok_or("restart settings")?
                .profile()
                .language,
            "fr"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn staged_pre_replace_failure_retains_cache_and_cleans_owned_temp()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let directory = roots.profile().join("native");
        let store = NativeProfileStore::create_fresh(&directory)?;
        let before = store.update(|_| Ok(()))?;
        // Inject after a real staged write+file sync, before rename. This checks
        // cleanup and cache evidence, not a physical disk-fault qualification.
        let bytes = fs::read(directory.join(SETTINGS))?;
        let result = store.update_with_checkpoints(
            |p| {
                p.language = "fr".into();
                Ok(())
            },
            |path| {
                assert_eq!(
                    ProfileDocument::decode(
                        &fs::read(path).map_err(|e| StorageError::io("read actual staging", e))?
                    )?
                    .profile()
                    .language,
                    "fr"
                );
                Err(StorageError::io(
                    "injected pre-replace failure",
                    std::io::Error::other("publication refused"),
                ))
            },
            backup::sync_directory,
        );
        assert!(matches!(result, Err(StorageError::Io { .. })));
        assert_eq!(store.load()?.ok_or("missing profile")?.profile(), &before);
        assert_eq!(fs::read(directory.join(SETTINGS))?, bytes);
        assert_eq!(fs::read_dir(&directory)?.count(), 2);
        Ok(())
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn profile_parent_race_cannot_redirect_staging_or_owned_temp_cleanup()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let directory = roots.profile().join("native");
        let retained = roots.profile().join("retained-native");
        let outside = roots.library();
        fs::write(outside.join(SETTINGS), b"foreign untouched")?;
        let store = NativeProfileStore::create_fresh(&directory)?;
        let before = store.update(|_| Ok(()))?;
        let result = store.update_with_checkpoints(
            |p| {
                p.language = "fr".into();
                Ok(())
            },
            |_| {
                fs::rename(&directory, &retained)
                    .and_then(|()| std::os::unix::fs::symlink(outside, &directory))
                    .map_err(|e| StorageError::io("inject native profile parent race", e))
            },
            |_| Ok(()),
        );
        assert!(matches!(result, Err(StorageError::PathEscape)));
        assert_eq!(fs::read(outside.join(SETTINGS))?, b"foreign untouched");
        assert_eq!(fs::read_dir(outside)?.count(), 1);
        assert_eq!(fs::read_dir(&retained)?.count(), 2);
        fs::remove_file(&directory)?;
        fs::rename(&retained, &directory)?;
        assert_eq!(
            store.load()?.ok_or("missing unchanged profile")?.profile(),
            &before
        );
        Ok(())
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn post_replace_parent_race_retains_actual_witnesses_and_poison_until_restart()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let directory = roots.profile().join("native");
        let retained = roots.profile().join("retained-native");
        let outside = roots.library();
        fs::write(outside.join(SETTINGS), b"foreign untouched")?;
        let store = NativeProfileStore::create_fresh(&directory)?;
        store.update(|_| Ok(()))?;
        let result = store.update_with_flush(
            |p| {
                p.language = "fr".into();
                Ok(())
            },
            |_| {
                fs::rename(&directory, &retained)
                    .and_then(|()| std::os::unix::fs::symlink(outside, &directory))
                    .map_err(|e| StorageError::io("inject post-replace parent race", e))
            },
        );
        assert!(matches!(
            result,
            Err(StorageError::ProfilePublicationUncertain { .. })
        ));
        assert!(matches!(
            store.load(),
            Err(StorageError::ProfileRestartRequired)
        ));
        assert_eq!(
            store
                .state
                .lock()
                .map_err(|_| "poisoned test mutex")?
                .as_ref()
                .ok_or("busy state")?
                .recovery_witnesses
                .len(),
            2
        );
        assert_eq!(fs::read(outside.join(SETTINGS))?, b"foreign untouched");
        assert_eq!(
            read_legacy(&retained)?
                .ok_or("missing actual published profile")?
                .profile()
                .language,
            "fr"
        );
        fs::remove_file(&directory)?;
        fs::rename(&retained, &directory)?;
        assert!(matches!(
            store.update(|_| Ok(())),
            Err(StorageError::ProfileRestartRequired)
        ));
        drop(store);
        assert_eq!(
            NativeProfileStore::open_native(&directory)?
                .load()?
                .ok_or("missing restart profile")?
                .profile()
                .language,
            "fr"
        );
        Ok(())
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn config_stage_cleanup_failure_keeps_primary_error_and_foreign_bytes()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let directory = roots.profile().join("native");
        let retained = directory.join("retained-owned-temp");
        let store = NativeProfileStore::create_fresh(&directory)?;
        store.update(|_| Ok(()))?;
        let result = store.update_with_checkpoints(
            |p| {
                p.language = "fr".into();
                Ok(())
            },
            |path| {
                fs::rename(path, &retained)
                    .and_then(|()| fs::write(path, b"foreign stage substitution"))
                    .map_err(|e| StorageError::io("inject stage substitution", e))?;
                Err(StorageError::io(
                    "primary publication refusal",
                    std::io::Error::other("test primary"),
                ))
            },
            |_| Ok(()),
        );
        let Err(StorageError::OwnedTempCleanupFailed { work, cleanup }) = result else {
            return Err("missing independent work/cleanup error".into());
        };
        assert!(matches!(
            *work,
            StorageError::Io {
                operation: "primary publication refusal",
                ..
            }
        ));
        assert!(matches!(*cleanup, StorageError::SnapshotChanged));
        assert!(matches!(
            store.load(),
            Err(StorageError::ProfileRestartRequired)
        ));
        assert_eq!(
            read_legacy(&directory)?
                .ok_or("missing original")?
                .profile()
                .language,
            "en"
        );
        assert_eq!(
            ProfileDocument::decode(&fs::read(&retained)?)?
                .profile()
                .language,
            "fr"
        );
        let foreign_before = fs::read_dir(&directory)?
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| fs::read(entry.path()).ok())
            .filter(|bytes| bytes == b"foreign stage substitution")
            .count();
        assert_eq!(foreign_before, 1);
        drop(store);
        let reopened = NativeProfileStore::open_native(&directory)?;
        assert_eq!(
            reopened
                .load()?
                .ok_or("missing restart profile")?
                .profile()
                .language,
            "en"
        );
        assert_eq!(fs::read_dir(&directory)?.count(), 4);
        Ok(())
    }

    #[test]
    fn profile_serialization_preserves_go_field_order_and_html_escaping()
    -> std::result::Result<(), Box<dyn Error>> {
        let mut profile = Profile::setup();
        profile.root = "Été<&>\u{2028}\u{2029}".into();
        profile.library_id = "RAW-ID".into();
        assert_eq!(
            String::from_utf8(encode(&profile)?)?,
            "{\n  \"version\": 1,\n  \"language\": \"en\",\n  \"root\": \"Été\\u003c\\u0026\\u003e\\u2028\\u2029\",\n  \"libraryId\": \"RAW-ID\",\n  \"complete\": false\n}"
        );
        profile.recovering = true;
        assert!(String::from_utf8(encode(&profile)?)?.ends_with("  \"recovering\": true\n}"));
        Ok(())
    }
    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn admitted_profile_update_owns_state_without_holding_mutex_or_admitting_peer()
    -> std::result::Result<(), Box<dyn Error>> {
        use std::sync::mpsc;
        let roots = TestRoots::new()?;
        let store = NativeProfileStore::create_fresh(&roots.profile().join("native"))?;
        store.update(|_| Ok(()))?;
        let (entered, observed) = mpsc::sync_channel(1);
        let (resume, wait) = mpsc::sync_channel(1);
        std::thread::scope(|scope| -> std::result::Result<(), Box<dyn Error>> {
            let store = &store;
            let update = scope.spawn(move || {
                store.update(|profile| {
                    // Callback execution owns the state, never its admission mutex.
                    let slot = store
                        .state
                        .try_lock()
                        .map_err(|_| StorageError::RuntimeFenced)?;
                    if slot.is_some() {
                        return Err(StorageError::CompatibilityInvariant);
                    }
                    drop(slot);
                    entered.send(()).map_err(|_| StorageError::WorkerStopped)?;
                    wait.recv().map_err(|_| StorageError::WorkerStopped)?;
                    profile.language = "fr".into();
                    Ok(())
                })
            });
            observed.recv()?;
            assert!(matches!(store.load(), Err(StorageError::ProfileBusy)));
            assert!(matches!(
                store.update(|_| Ok(())),
                Err(StorageError::ProfileBusy)
            ));
            resume.send(())?;
            assert_eq!(
                update.join().map_err(|_| "update panicked")??.language,
                "fr"
            );
            Ok(())
        })?;
        assert_eq!(store.load()?.ok_or("profile")?.profile().language, "fr");
        Ok(())
    }
}
