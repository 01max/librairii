//! Durable settings commands. Native views/menus consume their bounded projection.
use librairii_app::localization::Language;
use librairii_storage::{Result, StorageError, profile::NativeProfileStore};
use librairii_ui::settings::SettingsPresentation;

/// This runs on the settings worker with exclusive access to the presentation
/// owner. NativeProfileStore serializes against root commitment and merges the
/// newest independent language rather than publishing an old profile snapshot.
pub fn change_language(
    store: &NativeProfileStore,
    presentation: &mut SettingsPresentation,
    requested: &str,
) -> Result<()> {
    let language = Language::explicit(requested).map_err(|_| StorageError::InvalidProfile)?;
    let change = presentation
        .prepare_language(language)
        .map_err(|_| StorageError::InvalidProfile)?;
    store.update(|profile| {
        profile.language = language.code().into();
        Ok(())
    })?;
    presentation.language_persisted(change);
    Ok(())
}

pub fn restore_language(
    store: &NativeProfileStore,
    system: Language,
) -> Result<SettingsPresentation> {
    let language = match store.load()? {
        Some(document) => Language::explicit(&document.profile().language)
            .map_err(|_| StorageError::InvalidProfile)?,
        None => system,
    };
    SettingsPresentation::new(language).map_err(|_| StorageError::InvalidProfile)
}
