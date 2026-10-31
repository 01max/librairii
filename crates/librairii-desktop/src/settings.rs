//! Durable settings commands. Native views/menus consume their bounded projection.
use librairii_app::localization::Language;
use librairii_storage::{Result, StorageError, profile::NativeProfileStore};
use librairii_ui::settings::SettingsPresentation;
mod system_language;
pub use system_language::{
    LanguagePlatform, NativeLanguageSource, SystemLanguageSource, detect_language, system_language,
};

/// Read system preferences only for an installation without a saved choice.
/// Overrides call restore_language with English and never enter this route.
pub fn restore_from_system(
    store: &NativeProfileStore,
    source: &impl SystemLanguageSource,
) -> Result<SettingsPresentation> {
    if let Some(document) = store.load()? {
        let language = Language::explicit(&document.profile().language)
            .map_err(|_| StorageError::InvalidProfile)?;
        return SettingsPresentation::new(language).map_err(|_| StorageError::InvalidProfile);
    }
    restore_language(store, detect_language(LanguagePlatform::current(), source))
}

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
