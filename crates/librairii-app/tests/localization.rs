use librairii_app::localization::{Argument, Catalogs, Language, OutcomeStatus};
use std::error::Error;
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn only_two_explicit_languages_and_ordered_supported_system_preferences() -> TestResult {
    assert_eq!(Language::CHOICES, [Language::English, Language::French]);
    for code in ["", "de", "fr-CA", "FR", "system"] {
        assert!(Language::explicit(code).is_err());
    }
    assert_eq!(Language::explicit("fr")?, Language::French);
    assert_eq!(
        Language::from_system(["fr_CA.UTF-8", "en"]),
        Language::French
    );
    assert_eq!(Language::from_system(["de-DE", "fr"]), Language::English);
    assert_eq!(Language::from_system(["en-GB", "fr"]), Language::English);
    assert_eq!(Language::from_system(["ja", "ar"]), Language::English);
    assert_eq!(Language::from_system([]), Language::English);
    Ok(())
}

#[test]
fn embedded_catalogs_render_numbers_plural_forms_and_authored_parameters() -> TestResult {
    let catalogs = Catalogs::embedded()?;
    assert_eq!(catalogs.text(Language::French, "Settings…")?, "Réglages…");
    assert_eq!(Language::English.number(1234567), "1,234,567");
    assert_eq!(
        Language::French.number(-1234567),
        "-1\u{202f}234\u{202f}567"
    );
    assert_eq!(
        Language::English.number(i64::MIN),
        "-9,223,372,036,854,775,808"
    );
    assert!(Language::French.singular(0));
    assert!(!Language::English.singular(0));
    assert!(Language::French.singular(1));
    assert!(!Language::French.singular(2));
    assert_eq!(
        catalogs.render(
            Language::French,
            "Close {0}",
            &[Argument::Text("Mon histoire {1}")]
        )?,
        "Fermer Mon histoire {1}"
    );
    assert_eq!(
        catalogs.render(
            Language::French,
            "Official metadata refreshed; {0} local stories matched.",
            &[Argument::Number(1234)]
        )?,
        "Métadonnées officielles actualisées ; 1\u{202f}234 histoires locales associées."
    );
    assert!(
        catalogs
            .render(Language::English, "Close {0}", &[])
            .is_err()
    );
    assert!(
        catalogs
            .text(Language::English, "a made-up message identity")
            .is_err()
    );
    Ok(())
}

#[test]
fn localized_dates_use_the_supplied_offset_and_locale_without_mutating_the_instant() -> TestResult {
    let date = time::OffsetDateTime::parse(
        "2026-10-25T13:29:00+01:00",
        &time::format_description::well_known::Rfc3339,
    )?;
    assert_eq!(Language::English.date(date), "Oct 25, 2026, 1:29 PM");
    assert_eq!(Language::French.date(date), "25 oct. 2026, 13:29");
    let midnight = date.replace_time(time::Time::MIDNIGHT);
    assert_eq!(Language::English.date(midnight), "Oct 25, 2026, 12:00 AM");
    assert_eq!(date.offset().whole_hours(), 1);
    Ok(())
}

#[test]
fn historical_results_translate_known_messages_and_keep_unknown_original_details() -> TestResult {
    let catalogs = Catalogs::embedded()?;
    let raw = "Future successful export {0}";
    let result = catalogs.historical(Language::French, raw, OutcomeStatus::Succeeded)?;
    assert_eq!(result.summary, "Terminé");
    assert_eq!(result.technical_detail, Some(raw));
    let english = catalogs.historical(Language::English, raw, OutcomeStatus::Succeeded)?;
    assert_eq!(english.summary, raw);
    assert_eq!(english.technical_detail, Some(raw));
    assert_eq!(
        catalogs
            .historical(Language::English, "Annuler", OutcomeStatus::Cancelled)?
            .summary,
        "Undo"
    );
    assert_eq!(
        catalogs
            .historical(Language::English, "Terminé", OutcomeStatus::Succeeded)?
            .summary,
        "Done"
    );
    assert_eq!(
        catalogs
            .historical(
                Language::French,
                "Story imported.",
                OutcomeStatus::Succeeded
            )?
            .summary,
        "Histoire importée."
    );
    assert_eq!(
        catalogs
            .historical(
                Language::English,
                "Histoire importée.",
                OutcomeStatus::Succeeded
            )?
            .summary,
        "Story imported."
    );
    assert_eq!(
        catalogs
            .historical(
                Language::French,
                "Official metadata refreshed; 1234 local stories matched.",
                OutcomeStatus::Succeeded
            )?
            .summary,
        "Métadonnées officielles actualisées ; 1\u{202f}234 histoires locales associées."
    );
    for status in [
        OutcomeStatus::Succeeded,
        OutcomeStatus::Partial,
        OutcomeStatus::Skipped,
        OutcomeStatus::Conflicted,
        OutcomeStatus::Cancelled,
        OutcomeStatus::Interrupted,
        OutcomeStatus::Queued,
        OutcomeStatus::Running,
        OutcomeStatus::Failed,
        OutcomeStatus::Unknown,
    ] {
        for language in Language::CHOICES {
            assert!(
                !catalogs
                    .historical(language, raw, status)?
                    .summary
                    .is_empty()
            );
        }
    }
    Ok(())
}
