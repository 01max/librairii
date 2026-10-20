//! Independent version-1 profile data. Reading a profile never grants library authority.
//!
//! Legacy settings are read-only inputs. Native updates require a fresh, separate
//! namespace with its marker locked for the entire store lifetime. This lock does
//! not exclude an unmodified Go library process or authorize library adoption.
use crate::{Result, StorageError, backup};
use serde::{Deserialize, Serialize, de};
use std::{
    fmt,
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const MAX_PROFILE_BYTES: usize = 64 * 1024;
const SETTINGS: &str = "settings.json";
const MARKER: &str = ".librairii-native-profile";
const MARKER_BYTES: &[u8] = b"librairii-native-profile-v1\n";

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
    let mut bytes = Vec::new();
    file.take((MAX_PROFILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| StorageError::io("read bounded settings", e))?;
    Ok(Some((ProfileDocument::decode(&bytes)?, metadata)))
}

#[derive(Debug, Default)]
struct State {
    loaded: bool,
    document: Option<ProfileDocument>,
    settings_metadata: Option<Metadata>,
    poisoned: bool,
}

/// Serialized updates in a separately initialized native namespace. The marker
/// recognizes the namespace format, not trust in a copied library or journal.
///
/// Namespace/file witnesses are checked before effects. Descriptor-relative
/// mutation and adversarial ancestor races remain the distinct task4.8/4.9
/// capability boundary; do not use this fresh-root API as production adoption.
#[derive(Debug)]
pub struct NativeProfileStore {
    directory: PathBuf,
    directory_metadata: Metadata,
    marker: File,
    marker_metadata: Metadata,
    state: Mutex<State>,
}

impl NativeProfileStore {
    /// Initialize only a fresh explicit directory, never a legacy settings
    /// directory. Failure leaves any created namespace bytes for inspection.
    pub fn create_fresh(directory: &Path) -> Result<Self> {
        backup::require_directory_durability()?;
        if !directory.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        match fs::symlink_metadata(directory) {
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
            }
            Err(error) => return Err(StorageError::io("inspect profile namespace", error)),
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut marker = options
            .open(directory.join(MARKER))
            .map_err(|e| StorageError::io("create native profile marker", e))?;
        marker
            .write_all(MARKER_BYTES)
            .and_then(|()| marker.sync_all())
            .map_err(|e| StorageError::io("flush native profile marker", e))?;
        backup::sync_directory(directory)?;
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
        let mut marker = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| StorageError::io("open native profile marker", e))?;
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
            directory: directory.to_path_buf(),
            directory_metadata,
            marker,
            marker_metadata,
            state: Mutex::new(State::default()),
        };
        store.revalidate_namespace()?;
        Ok(store)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn recovery_directory(&self) -> PathBuf {
        self.directory.join("recovery")
    }

    pub fn load(&self) -> Result<Option<ProfileDocument>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)?;
        self.load_locked(&mut state)?;
        Ok(state.document.clone())
    }

    /// The closure edits data only. It must be bounded and synchronous, and
    /// must not re-enter this store. A selected root still needs independent
    /// runtime adoption/identity/lease verification before any library effects.
    pub fn update(&self, change: impl FnOnce(&mut Profile) -> Result<()>) -> Result<Profile> {
        self.update_with_flush(change, backup::sync_directory)
    }

    fn load_locked(&self, state: &mut State) -> Result<()> {
        if state.poisoned {
            return Err(StorageError::ProfileRestartRequired);
        }
        self.revalidate_namespace()?;
        if !state.loaded {
            if let Some((document, metadata)) =
                read_document_with_witness(&self.directory.join(SETTINGS))?
            {
                state.document = Some(document);
                state.settings_metadata = Some(metadata);
            }
            state.loaded = true;
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
        let mut state = self
            .state
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)?;
        self.load_locked(&mut state)?;
        self.revalidate_settings(&state)?;
        let mut next = state
            .document
            .as_ref()
            .map_or_else(Profile::setup, |d| d.profile.clone());
        change(&mut next)?;
        next.validate()?;
        let bytes = encode(&next)?;
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
            state.poisoned = true;
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

    fn revalidate_settings(&self, state: &State) -> Result<()> {
        match (
            read_document_with_witness(&self.directory.join(SETTINGS))?,
            &state.settings_metadata,
        ) {
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
        let marker = fs::symlink_metadata(self.directory.join(MARKER))
            .map_err(|e| StorageError::io("revalidate profile marker", e))?;
        require_regular(&marker)?;
        require_same(&self.marker_metadata, &marker)?;
        let mut bytes = Vec::new();
        File::open(self.directory.join(MARKER))
            .map_err(|e| StorageError::io("reopen profile marker witness", e))?
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
}
