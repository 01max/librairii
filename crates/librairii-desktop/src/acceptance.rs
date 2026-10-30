//! Explicit packaged GUI fixtures. Admission precedes menus, profiles and UI.
use crate::{
    presentation::Command,
    startup::{NativeEnvironment, StartupEnvironment},
};
use librairii_adapters::lunii::CatalogClient;
use librairii_storage::{
    StorageError,
    profile::{ProfileLocations, read_legacy},
};
use std::{
    ffi::OsString,
    fs,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    time::Duration,
};
pub const MARKER: &str = ".librairii-packaged-fixture";
pub const MARKER_BYTES: &[u8] = b"librairii-packaged-acceptance-v1\n";
const VARIABLES: [&str; 4] = [
    "LIBRAIRII_ACCEPTANCE_ROOT",
    "LIBRAIRII_ACCEPTANCE_PROFILE",
    "LIBRAIRII_ACCEPTANCE_LIBRARY",
    "LIBRAIRII_ACCEPTANCE_CATALOG",
];
fn refused() -> io::Error {
    io::Error::other(
        "packaged acceptance requires explicit owned isolated roots and a loopback catalog",
    )
}
#[derive(Clone, Debug)]
pub struct Scope {
    root: PathBuf,
}
impl Scope {
    fn revalidate(&self) -> io::Result<()> {
        if fs::canonicalize(&self.root)? != self.root
            || fs::symlink_metadata(&self.root)?.file_type().is_symlink()
        {
            return Err(refused());
        }
        let marker = self.root.join(MARKER);
        let metadata = fs::symlink_metadata(&marker)?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(refused());
        }
        let mut bytes = Vec::new();
        fs::File::open(marker)?.take(64).read_to_end(&mut bytes)?;
        if bytes != MARKER_BYTES {
            return Err(refused());
        }
        Ok(())
    }
    fn lexical(&self, path: &Path) -> bool {
        path.is_absolute()
            && path != self.root
            && path.starts_with(&self.root)
            && path
                .components()
                .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
    }
    fn physical(&self, path: &Path) -> bool {
        if !self.lexical(path) || self.revalidate().is_err() {
            return false;
        }
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
        };
        let mut cursor = self.root.clone();
        for part in relative.components() {
            cursor.push(part);
            match fs::symlink_metadata(&cursor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink()
                        || (metadata.is_file() && metadata.nlink() != 1)
                    {
                        return false;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
                Err(_) => return false,
            }
        }
        true
    }
    fn paths(command: &Command, mut check: impl FnMut(&Path) -> bool) -> bool {
        use crate::{maintenance::Action as Settings, workflows};
        match command {
            Command::Maintenance {
                action:
                    Settings::Select(path)
                    | Settings::Locate(path)
                    | Settings::Diagnostics(path)
                    | Settings::ImportPreferences(path),
                ..
            }
            | Command::Exports {
                action: workflows::exports::Action::Destination(path),
                ..
            } => check(path),
            Command::Operations {
                action: workflows::operations::Action::Import(paths),
                ..
            } => paths.iter().all(|path| check(path)),
            _ => true,
        }
    }
    /// Pure path checks on UI intake; filesystem work stays on the controller.
    pub(crate) fn admits(&self, command: &Command) -> bool {
        Self::paths(command, |path| self.lexical(path))
    }
    pub(crate) fn recheck(&self, command: &Command) -> bool {
        Self::paths(command, |path| self.physical(path))
    }
    fn locations(&self, locations: &ProfileLocations) -> librairii_storage::Result<()> {
        for path in [
            Some(&locations.native),
            Some(&locations.data),
            locations.legacy.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if !self.physical(path) {
                return Err(StorageError::InvalidProfile);
            }
        }
        for path in [Some(&locations.native), locations.legacy.as_ref()]
            .into_iter()
            .flatten()
        {
            if let Some(document) = read_legacy(path)? {
                let configured = &document.profile().root;
                if !configured.is_empty() && !self.physical(Path::new(configured)) {
                    return Err(StorageError::InvalidProfile);
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
struct Fixture {
    scope: Scope,
    locations: ProfileLocations,
    catalog: CatalogClient,
}
#[derive(Debug)]
pub struct ProcessEnvironment {
    fixture: Option<Fixture>,
}
impl ProcessEnvironment {
    pub fn read(environment: &impl StartupEnvironment) -> io::Result<Self> {
        let mode = environment.variable("LIBRAIRII_PACKAGED_ACCEPTANCE");
        if mode.is_none() {
            if VARIABLES
                .iter()
                .any(|name| environment.variable(name).is_some())
            {
                return Err(refused());
            }
            return Ok(Self { fixture: None });
        }
        if mode.as_deref() != Some(std::ffi::OsStr::new("1"))
            || environment
                .variable(librairii_storage::DATA_ROOT_OVERRIDE)
                .is_some()
        {
            return Err(refused());
        }
        let required = |name| environment.variable(name).ok_or_else(refused);
        let root = PathBuf::from(required(VARIABLES[0])?);
        let profile = PathBuf::from(required(VARIABLES[1])?);
        let library = PathBuf::from(required(VARIABLES[2])?);
        let temporary = fs::canonicalize(std::env::temp_dir())?;
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(refused)?;
        if root.parent() != Some(temporary.as_path())
            || !(name.starts_with("librairii-packaged-")
                || name.starts_with("librairii-synthetic-"))
            || profile != root.join("profile")
            || library != root.join("library")
        {
            return Err(refused());
        }
        let scope = Scope { root };
        scope.revalidate()?;
        for path in [&profile, &library] {
            if !path.is_dir() || !scope.physical(path) {
                return Err(refused());
            }
        }
        let mut locations =
            ProfileLocations::for_host(&profile, None, None).map_err(|_| refused())?;
        locations.data = library.join(locations.data.file_name().ok_or_else(refused)?);
        scope.locations(&locations).map_err(|_| refused())?;
        let endpoint = required(VARIABLES[3])?;
        let catalog = CatalogClient::loopback(
            endpoint.to_str().ok_or_else(refused)?,
            Duration::from_secs(2),
        )
        .map_err(|_| refused())?;
        Ok(Self {
            fixture: Some(Fixture {
                scope,
                locations,
                catalog,
            }),
        })
    }
}
impl StartupEnvironment for ProcessEnvironment {
    fn variable(&self, name: &str) -> Option<OsString> {
        if self.fixture.is_some() {
            None
        } else {
            NativeEnvironment.variable(name)
        }
    }
    fn catalog_client(&self) -> Option<CatalogClient> {
        self.fixture.as_ref().map(|fixture| fixture.catalog.clone())
    }
    fn profile_locations(&self) -> librairii_storage::Result<Option<ProfileLocations>> {
        self.fixture
            .as_ref()
            .map(|fixture| {
                fixture.scope.locations(&fixture.locations)?;
                Ok(fixture.locations.clone())
            })
            .transpose()
    }
    fn acceptance_scope(&self) -> Option<Scope> {
        self.fixture.as_ref().map(|fixture| fixture.scope.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        maintenance::Action,
        presentation::{OfferError, intake::Intake},
    };
    use librairii_testkit::TestRoots;
    use std::collections::BTreeMap;
    type Result = std::result::Result<(), Box<dyn std::error::Error>>;
    struct Environment(BTreeMap<&'static str, OsString>);
    impl StartupEnvironment for Environment {
        fn variable(&self, name: &str) -> Option<OsString> {
            assert_ne!(
                name, "HOME",
                "fixture admission must precede home discovery"
            );
            self.0.get(name).cloned()
        }
    }
    fn fixture(roots: &TestRoots) -> io::Result<Environment> {
        let root = roots.library().parent().ok_or_else(refused)?;
        fs::write(root.join(MARKER), MARKER_BYTES)?;
        Ok(Environment(BTreeMap::from([
            ("LIBRAIRII_PACKAGED_ACCEPTANCE", "1".into()),
            (VARIABLES[0], root.as_os_str().to_owned()),
            (VARIABLES[1], roots.profile().as_os_str().to_owned()),
            (VARIABLES[2], roots.library().as_os_str().to_owned()),
            (VARIABLES[3], "http://127.0.0.1:9/".into()),
        ])))
    }
    #[test]
    fn packaged_context_requires_owned_roots_and_loopback_before_profile_discovery() -> Result {
        let roots = TestRoots::new()?;
        let mut environment = fixture(&roots)?;
        for name in VARIABLES {
            let original = environment.0.remove(name).ok_or("fixture variable")?;
            assert!(ProcessEnvironment::read(&environment).is_err());
            environment.0.insert(name, original);
        }
        let context = ProcessEnvironment::read(&environment)?;
        assert!(context.variable("HOME").is_none());
        assert!(context.catalog_client().is_some());
        let locations = context.profile_locations()?.ok_or("isolated locations")?;
        assert!(locations.native.starts_with(roots.profile()));
        assert!(locations.data.starts_with(roots.library()));
        for endpoint in [
            "https://server-data-prod.lunii.com/",
            "http://localhost:9/",
            "http://192.0.2.1:9/",
        ] {
            environment.0.insert(VARIABLES[3], endpoint.into());
            assert!(ProcessEnvironment::read(&environment).is_err());
        }
        environment
            .0
            .insert(VARIABLES[3], "http://127.0.0.1:9/".into());
        environment.0.insert(
            librairii_storage::DATA_ROOT_OVERRIDE,
            roots.library().into(),
        );
        assert!(ProcessEnvironment::read(&environment).is_err());
        assert_eq!(fs::read_dir(roots.profile())?.count(), 0);
        assert_eq!(fs::read_dir(roots.library())?.count(), 0);
        Ok(())
    }
    #[test]
    fn foreign_persisted_library_and_fixture_alias_refuse_without_external_access() -> Result {
        let roots = TestRoots::new()?;
        let external = TestRoots::new()?;
        let environment = fixture(&roots)?;
        let context = ProcessEnvironment::read(&environment)?;
        let locations = context.profile_locations()?.ok_or("isolated locations")?;
        fs::create_dir_all(&locations.native)?;
        fs::write(external.library().join("preserved"), b"foreign data")?;
        fs::write(
            locations.native.join("settings.json"),
            serde_json::to_vec(&serde_json::json!({
                "version":1,"language":"en","root":external.library(),
            "libraryId":"a0000000-0000-4000-8000-000000000018","complete":true
            }))?,
        )?;
        assert_eq!(
            read_legacy(&locations.native)?
                .ok_or("valid foreign profile")?
                .profile()
                .root,
            external.library().to_str().ok_or("fixture path")?
        );
        assert!(ProcessEnvironment::read(&environment).is_err());
        fs::remove_file(locations.native.join("settings.json"))?;
        fs::rename(
            roots.library(),
            roots.library().with_file_name("retained-library"),
        )?;
        std::os::unix::fs::symlink(external.library(), roots.library())?;
        assert!(ProcessEnvironment::read(&environment).is_err());
        assert_eq!(
            fs::read(external.library().join("preserved"))?,
            b"foreign data"
        );
        Ok(())
    }
    #[test]
    fn intake_rejects_external_paths_and_rechecks_owned_picker_paths_off_ui_thread() -> Result {
        let roots = TestRoots::new()?;
        let external = TestRoots::new()?;
        let context = ProcessEnvironment::read(&fixture(&roots)?)?;
        let intake = Intake::with_scope(context.acceptance_scope());
        let select = |path: PathBuf| Command::Maintenance {
            generation: None,
            action: Action::Select(path),
        };
        assert_eq!(
            intake.offer(select(external.library().into())),
            Err(OfferError::Closed)
        );
        assert_eq!(intake.latest(), 0);
        let selected = roots.library().join("new-library");
        assert_eq!(intake.offer(select(selected.clone())), Ok(1));
        std::os::unix::fs::symlink(external.library(), selected)?;
        assert!(matches!(intake.receive(), Err(OfferError::Closed)));
        assert_eq!(fs::read_dir(external.library())?.count(), 0);
        assert_eq!(intake.offer(Command::Refresh), Ok(2));
        assert!(intake.receive()?.is_some());
        Ok(())
    }
}
