//! Native preference persistence or explicitly isolated, session-only values.
//! Memory preferences never resolve a home/configuration path or library root.
use librairii_app::localization::Language;
use librairii_core::{LibraryId, navigation::Navigation, shelves::WarningAcknowledgements};
use librairii_storage::{
    Result, StorageError,
    profile::{InterfacePreferences, NativeProfileStore},
};
use std::sync::Mutex;
#[derive(Debug, Default)]
pub struct Memory {
    navigation: Mutex<Option<Vec<u8>>>,
    interface: Mutex<Option<InterfacePreferences>>,
    warnings: Mutex<Option<Vec<u8>>>,
}
pub trait Preferences {
    fn require_alive(&self) -> Result<()>;
    fn save_language(
        &self,
        presentation: &mut librairii_ui::settings::SettingsPresentation,
        requested: Language,
    ) -> Result<()>;
    fn load_navigation(&self, id: LibraryId, generation: u64) -> Result<Option<Navigation>>;
    fn save_navigation(&self, value: &Navigation) -> Result<()>;
    fn load_interface(&self, id: LibraryId) -> Result<Option<InterfacePreferences>>;
    fn save_interface(&self, value: &InterfacePreferences) -> Result<()>;
    fn load_shelf_warnings(&self, id: LibraryId) -> Result<Option<WarningAcknowledgements>>;
    fn save_shelf_warnings(&self, value: &WarningAcknowledgements) -> Result<()>;
}
impl Preferences for NativeProfileStore {
    fn save_language(
        &self,
        presentation: &mut librairii_ui::settings::SettingsPresentation,
        requested: Language,
    ) -> Result<()> {
        crate::settings::change_language(self, presentation, requested.code())
    }
    fn require_alive(&self) -> Result<()> {
        self.load().map(|_| ())
    }
    fn load_navigation(&self, id: LibraryId, generation: u64) -> Result<Option<Navigation>> {
        NativeProfileStore::load_navigation(self, id, generation)
    }
    fn save_navigation(&self, v: &Navigation) -> Result<()> {
        NativeProfileStore::save_navigation(self, v)
    }
    fn load_interface(&self, id: LibraryId) -> Result<Option<InterfacePreferences>> {
        NativeProfileStore::load_interface(self, id)
    }
    fn save_interface(&self, v: &InterfacePreferences) -> Result<()> {
        NativeProfileStore::save_interface(self, v)
    }
    fn load_shelf_warnings(&self, id: LibraryId) -> Result<Option<WarningAcknowledgements>> {
        NativeProfileStore::load_shelf_warnings(self, id)
    }
    fn save_shelf_warnings(&self, v: &WarningAcknowledgements) -> Result<()> {
        NativeProfileStore::save_shelf_warnings(self, v)
    }
}
impl Preferences for Memory {
    fn save_language(
        &self,
        presentation: &mut librairii_ui::settings::SettingsPresentation,
        requested: Language,
    ) -> Result<()> {
        let change = presentation
            .prepare_language(requested)
            .map_err(|_| StorageError::InvalidProfile)?;
        presentation.language_persisted(change);
        Ok(())
    }
    fn require_alive(&self) -> Result<()> {
        Ok(())
    }
    fn load_navigation(&self, id: LibraryId, generation: u64) -> Result<Option<Navigation>> {
        self.navigation
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?
            .as_ref()
            .map(|b| {
                Navigation::decode(b, id, generation).map_err(|_| StorageError::InvalidNavigation)
            })
            .transpose()
    }
    fn save_navigation(&self, value: &Navigation) -> Result<()> {
        let bytes = value
            .encode()
            .map_err(|_| StorageError::InvalidNavigation)?;
        *self
            .navigation
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)? = Some(bytes);
        Ok(())
    }
    fn load_interface(&self, id: LibraryId) -> Result<Option<InterfacePreferences>> {
        let value = self
            .interface
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?
            .clone();
        if value
            .as_ref()
            .is_some_and(|v| v.library_id().ok() != Some(id))
        {
            return Err(StorageError::InvalidNavigation);
        }
        Ok(value)
    }
    fn save_interface(&self, value: &InterfacePreferences) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|_| StorageError::InvalidNavigation)?;
        InterfacePreferences::decode(&bytes, value.library_id()?)?;
        *self
            .interface
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)? = Some(value.clone());
        Ok(())
    }
    fn load_shelf_warnings(&self, id: LibraryId) -> Result<Option<WarningAcknowledgements>> {
        self.warnings
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?
            .as_ref()
            .map(|b| WarningAcknowledgements::decode(b, id).map_err(Into::into))
            .transpose()
    }
    fn save_shelf_warnings(&self, value: &WarningAcknowledgements) -> Result<()> {
        let bytes = value.encode()?;
        *self
            .warnings
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)? = Some(bytes);
        Ok(())
    }
}
