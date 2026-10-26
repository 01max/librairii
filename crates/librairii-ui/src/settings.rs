//! Settings presentation retained independently from library/query/editor/job state.
use librairii_app::localization::{Argument, Catalogs, Language, LocalizationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuLanguage {
    pub language: Language,
    pub revision: u64,
    pub settings: String,
}

#[derive(Debug)]
pub struct LanguageChange {
    menu: MenuLanguage,
}

/// A bounded main-thread projection. Native composition consumes the latest
/// menu revision and retains an early Settings command until readiness. This
/// owner contains no content, metadata, query, selection, editor or job state.
#[derive(Debug)]
pub struct SettingsPresentation {
    catalogs: Catalogs,
    current: MenuLanguage,
    pending_menu: bool,
    ready: bool,
    pending_open: bool,
}
impl SettingsPresentation {
    pub fn new(language: Language) -> Result<Self, LocalizationError> {
        let catalogs = Catalogs::embedded()?;
        let settings = catalogs.text(language, "Settings…")?.into();
        Ok(Self {
            catalogs,
            current: MenuLanguage {
                language,
                revision: 0,
                settings,
            },
            pending_menu: true,
            ready: false,
            pending_open: false,
        })
    }
    pub fn current(&self) -> &MenuLanguage {
        &self.current
    }
    pub fn text(&self, key: &str, values: &[Argument<'_>]) -> Result<String, LocalizationError> {
        self.catalogs.render(self.current.language, key, values)
    }
    /// Prepare all fallible presentation work before attempting persistence.
    pub fn prepare_language(
        &self,
        language: Language,
    ) -> Result<LanguageChange, LocalizationError> {
        Ok(LanguageChange {
            menu: MenuLanguage {
                language,
                revision: self
                    .current
                    .revision
                    .checked_add(1)
                    .ok_or(LocalizationError)?,
                settings: self.catalogs.text(language, "Settings…")?.into(),
            },
        })
    }
    /// Called by the composition controller only after durable settings success.
    /// Rendering reads the new language immediately; menu changes coalesce to
    /// one latest revision, including before native menu/window readiness.
    pub fn language_persisted(&mut self, change: LanguageChange) {
        self.current = change.menu;
        self.pending_menu = true;
    }
    pub fn take_menu_change(&mut self) -> Option<MenuLanguage> {
        if self.ready && std::mem::take(&mut self.pending_menu) {
            Some(self.current.clone())
        } else {
            None
        }
    }
    pub fn native_ready(&mut self) {
        self.ready = true;
    }
    pub fn request_open(&mut self) {
        self.pending_open = true;
    }
    pub fn take_open_request(&mut self) -> bool {
        self.ready && std::mem::take(&mut self.pending_open)
    }
}
