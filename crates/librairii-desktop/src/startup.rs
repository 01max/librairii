//! Environment selection is ordered: an explicit override bypasses every
//! production home/profile/default/system-language read and write.
use librairii_app::localization::Language;
use librairii_storage::{
    IsolatedLibrary, Result, StorageError,
    database::WorkCancellation,
    profile::ProfileLocations,
    settings::{Startup, inspect_startup},
};
use librairii_ui::settings::SettingsPresentation;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
pub trait StartupEnvironment {
    fn variable(&self, name: &str) -> Option<OsString>;
    /// Explicitly supplied owned test clients can target only the adapter's
    /// validated numeric-loopback route. Native production uses fixed endpoints.
    fn catalog_client(&self) -> Option<librairii_adapters::lunii::CatalogClient> {
        None
    }
}
#[derive(Debug, Default)]
pub struct NativeEnvironment;
impl StartupEnvironment for NativeEnvironment {
    fn variable(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }
}
#[derive(Debug)]
pub enum StartupSelection {
    Override(OverrideSettings),
    Production {
        locations: ProfileLocations,
        decision: Box<Startup>,
    },
}
#[derive(Debug)]
pub struct OverrideSettings {
    root: PathBuf,
    presentation: SettingsPresentation,
}
impl OverrideSettings {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn presentation(&self) -> &SettingsPresentation {
        &self.presentation
    }
    pub fn change_language(&mut self, requested: &str) -> Result<()> {
        let language = Language::explicit(requested).map_err(|_| StorageError::InvalidProfile)?;
        let change = self
            .presentation
            .prepare_language(language)
            .map_err(|_| StorageError::InvalidProfile)?;
        self.presentation.language_persisted(change);
        Ok(())
    }
    pub const fn relocation_allowed(&self) -> bool {
        false
    }
    pub const fn relocation_message(&self) -> &'static str {
        "settings.override"
    }
    /// Retains the existing isolated-override safety boundary: only a fresh
    /// explicitly selected development root, never profile-selected adoption.
    pub fn initialize(&self, id: &str) -> Result<IsolatedLibrary> {
        IsolatedLibrary::initialize_from_override(
            Some(self.root.as_os_str()),
            id.parse().map_err(|_| StorageError::InvalidIdentity)?,
        )
    }
}
pub fn select_startup(
    environment: &impl StartupEnvironment,
    cancel: &WorkCancellation,
) -> Result<StartupSelection> {
    select_startup_with_progress(environment, cancel, || Ok(()))
}
pub(crate) fn select_startup_with_progress(
    environment: &impl StartupEnvironment,
    cancel: &WorkCancellation,
    opening: impl FnOnce() -> Result<()>,
) -> Result<StartupSelection> {
    if let Some(root) = environment.variable(librairii_storage::DATA_ROOT_OVERRIDE) {
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        return Ok(StartupSelection::Override(OverrideSettings {
            root,
            presentation: SettingsPresentation::new(Language::English)
                .map_err(|_| StorageError::InvalidProfile)?,
        }));
    }
    let home = PathBuf::from(
        environment
            .variable("HOME")
            .ok_or(StorageError::InvalidProfile)?,
    );
    let config = environment.variable("XDG_CONFIG_HOME").map(PathBuf::from);
    let data = environment.variable("XDG_DATA_HOME").map(PathBuf::from);
    let locations = ProfileLocations::for_host(&home, config.as_deref(), data.as_deref())?;
    opening()?;
    let decision = inspect_startup(&locations, cancel);
    Ok(StartupSelection::Production {
        locations,
        decision: Box::new(decision),
    })
}
