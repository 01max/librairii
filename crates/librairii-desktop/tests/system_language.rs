#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::localization::Language;
use librairii_desktop::settings::{
    LanguagePlatform, SystemLanguageSource, detect_language, restore_from_system,
};
use librairii_storage::profile::NativeProfileStore;
use librairii_testkit::TestRoots;
use std::{cell::Cell, collections::BTreeMap, error::Error};

#[derive(Default)]
struct ObservedSystem {
    apple: Option<String>,
    variables: BTreeMap<String, String>,
    reads: Cell<usize>,
}
impl SystemLanguageSource for ObservedSystem {
    fn apple_languages(&self) -> Option<String> {
        self.reads.set(self.reads.get() + 1);
        self.apple.clone()
    }
    fn locale(&self, name: &str) -> Option<String> {
        self.reads.set(self.reads.get() + 1);
        self.variables.get(name).cloned()
    }
}

#[test]
fn os_language_adapter_retains_first_preference_and_frozen_variable_precedence() {
    let mut source = ObservedSystem {
        apple: Some("(\n    \"de-DE\",\n    \"fr-FR\"\n)".into()),
        ..Default::default()
    };
    assert_eq!(
        detect_language(LanguagePlatform::MacOs, &source),
        Language::English
    );
    source.apple = Some("(\"fr-CA\", \"en\")".into());
    assert_eq!(
        detect_language(LanguagePlatform::MacOs, &source),
        Language::French
    );
    source
        .variables
        .insert("LC_ALL".into(), "fr_FR.UTF-8".into());
    source.variables.insert("LANGUAGE".into(), "de:fr".into());
    assert_eq!(
        detect_language(LanguagePlatform::Linux, &source),
        Language::English
    );
    source.variables.remove("LANGUAGE");
    assert_eq!(
        detect_language(LanguagePlatform::Linux, &source),
        Language::French
    );
    source.variables.clear();
    assert_eq!(
        detect_language(LanguagePlatform::Linux, &source),
        Language::English
    );
}

#[test]
fn settings_startup_uses_os_adapter_only_until_a_saved_language_exists()
-> Result<(), Box<dyn Error>> {
    let roots = TestRoots::new()?;
    let store = NativeProfileStore::create_fresh(&roots.profile().join("settings"))?;
    let source = ObservedSystem {
        apple: Some("(fr-FR)".into()),
        variables: BTreeMap::from([("LANG".into(), "fr_FR.UTF-8".into())]),
        ..Default::default()
    };
    assert_eq!(
        restore_from_system(&store, &source)?.current().language,
        Language::French
    );
    assert!(source.reads.get() > 0);
    store.update(|profile| {
        profile.language = "en".into();
        Ok(())
    })?;
    let reads = source.reads.get();
    assert_eq!(
        restore_from_system(&store, &source)?.current().language,
        Language::English
    );
    assert_eq!(source.reads.get(), reads);
    Ok(())
}
