//! Embedded product catalogs and presentation policy; no persisted content is translated.
use librairii_core::{OperationState, operations::ItemState};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "fr")]
    French,
}
impl Language {
    pub const CHOICES: [Self; 2] = [Self::English, Self::French];
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::French => "fr",
        }
    }
    pub fn explicit(code: &str) -> Result<Self, LocalizationError> {
        match code {
            "en" => Ok(Self::English),
            "fr" => Ok(Self::French),
            _ => Err(LocalizationError),
        }
    }
    /// The first supported system preference wins. A saved explicit choice is
    /// resolved separately and never replaced by an operating-system change.
    pub fn from_system<'a>(preferences: impl IntoIterator<Item = &'a str>) -> Self {
        for preference in preferences {
            let primary = preference
                .trim()
                .split(['-', '_', '.', '@'])
                .next()
                .unwrap_or_default();
            if primary.eq_ignore_ascii_case("fr") {
                return Self::French;
            }
            if primary.eq_ignore_ascii_case("en") {
                return Self::English;
            }
        }
        Self::English
    }
    /// The two supported languages' cardinal one/other rules for integral counts.
    pub const fn singular(self, count: u64) -> bool {
        match self {
            Self::English => count == 1,
            Self::French => count <= 1,
        }
    }
    pub fn number(self, value: i64) -> String {
        let digits = value.unsigned_abs().to_string();
        let separator = match self {
            Self::English => ',',
            Self::French => '\u{202f}',
        };
        let mut result = String::with_capacity(digits.len() + digits.len() / 3 + 1);
        if value < 0 {
            result.push('-');
        }
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index).is_multiple_of(3) {
                result.push(separator);
            }
            result.push(digit);
        }
        result
    }
    /// Medium date / short time in an explicit observed offset. The caller
    /// supplies the OS-local offset; formatting does not change the instant or
    /// pretend UTC is the user's timezone.
    pub fn date(self, value: OffsetDateTime) -> String {
        let month = usize::from(u8::from(value.month())) - 1;
        match self {
            Self::English => {
                let months = [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ];
                let hour = match value.hour() % 12 {
                    0 => 12,
                    hour => hour,
                };
                format!(
                    "{} {}, {}, {}:{:02} {}",
                    months[month],
                    value.day(),
                    value.year(),
                    hour,
                    value.minute(),
                    if value.hour() < 12 { "AM" } else { "PM" }
                )
            }
            Self::French => {
                let months = [
                    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.",
                    "oct.", "nov.", "déc.",
                ];
                format!(
                    "{} {} {}, {:02}:{:02}",
                    value.day(),
                    months[month],
                    value.year(),
                    value.hour(),
                    value.minute()
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalizationError;
impl fmt::Display for LocalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("settings.language_invalid")
    }
}
impl std::error::Error for LocalizationError {}

#[derive(Clone, Copy, Debug)]
pub enum Argument<'a> {
    Text(&'a str),
    Number(i64),
}

/// Keys are the preserved product message identities. Both catalogs are linked
/// into the executable, independent of its working directory or installation.
#[derive(Debug)]
pub struct Catalogs {
    english: BTreeMap<String, String>,
    french: BTreeMap<String, String>,
}
impl Catalogs {
    pub fn embedded() -> Result<Self, LocalizationError> {
        let english: BTreeMap<String, String> = serde_json::from_str(include_str!(
            "../../../internal/localization/locales/en.json"
        ))
        .map_err(|_| LocalizationError)?;
        let french: BTreeMap<String, String> = serde_json::from_str(include_str!(
            "../../../internal/localization/locales/fr.json"
        ))
        .map_err(|_| LocalizationError)?;
        if english.is_empty()
            || !english.keys().eq(french.keys())
            || english
                .values()
                .chain(french.values())
                .any(String::is_empty)
        {
            return Err(LocalizationError);
        }
        Ok(Self { english, french })
    }
    pub fn text(&self, language: Language, key: &str) -> Result<&str, LocalizationError> {
        let catalog = match language {
            Language::English => &self.english,
            Language::French => &self.french,
        };
        catalog
            .get(key)
            .map(String::as_str)
            .ok_or(LocalizationError)
    }
    /// Scan the template once, so a brace sequence in authored input cannot be
    /// substituted as another argument. Text is plain native text, never markup.
    pub fn render(
        &self,
        language: Language,
        key: &str,
        values: &[Argument<'_>],
    ) -> Result<String, LocalizationError> {
        let template = self.text(language, key)?;
        let mut result = String::new();
        let mut remaining = template;
        while let Some(start) = remaining.find('{') {
            result.push_str(&remaining[..start]);
            let after = &remaining[start + 1..];
            let Some(end) = after.find('}') else {
                result.push_str(&remaining[start..]);
                return Ok(result);
            };
            if let Ok(index) = after[..end].parse::<usize>() {
                match values.get(index).ok_or(LocalizationError)? {
                    Argument::Text(text) => result.push_str(text),
                    Argument::Number(number) => result.push_str(&language.number(*number)),
                }
                remaining = &after[end + 1..];
            } else {
                result.push('{');
                remaining = after;
            }
        }
        result.push_str(remaining);
        Ok(result)
    }
    pub fn plural<'a>(
        &'a self,
        language: Language,
        count: u64,
        one: &str,
        other: &str,
    ) -> Result<&'a str, LocalizationError> {
        self.text(language, if language.singular(count) { one } else { other })
    }
    pub fn historical<'a>(
        &self,
        language: Language,
        message: &'a str,
        status: OutcomeStatus,
    ) -> Result<LocalizedOutcome<'a>, LocalizationError> {
        let known = self
            .english
            .get_key_value(message)
            .map(|(key, _)| key)
            .or_else(|| {
                self.french
                    .iter()
                    .find(|(_, text)| text.as_str() == message)
                    .map(|(key, _)| key)
            });
        if let Some(key) = known {
            return Ok(LocalizedOutcome {
                summary: self.text(language, key)?.into(),
                technical_detail: None,
            });
        }
        if let Some(count) = message
            .strip_prefix("Official metadata refreshed; ")
            .and_then(|s| s.strip_suffix(" local stories matched."))
            .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|s| s.parse::<i64>().ok())
        {
            return Ok(LocalizedOutcome {
                summary: self.render(
                    language,
                    "Official metadata refreshed; {0} local stories matched.",
                    &[Argument::Number(count)],
                )?,
                technical_detail: None,
            });
        }
        Ok(LocalizedOutcome {
            summary: self.text(language, status.key())?.into(),
            technical_detail: Some(message),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct LocalizedOutcome<'a> {
    pub summary: String,
    pub technical_detail: Option<&'a str>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeStatus {
    Succeeded,
    Partial,
    Skipped,
    Conflicted,
    Cancelled,
    Interrupted,
    Queued,
    Running,
    Failed,
    Unknown,
}
impl OutcomeStatus {
    const fn key(self) -> &'static str {
        match self {
            Self::Succeeded => "Completed",
            Self::Partial => "Completed with exceptions",
            Self::Skipped => "Skipped",
            Self::Conflicted => "Conflicted",
            Self::Cancelled => "Cancelled",
            Self::Interrupted => "Interrupted",
            Self::Queued => "Queued",
            Self::Running => "In progress",
            Self::Failed => "The action could not be completed",
            Self::Unknown => "Action result unavailable",
        }
    }
}
impl From<OperationState> for OutcomeStatus {
    fn from(state: OperationState) -> Self {
        match state {
            OperationState::Queued => Self::Queued,
            OperationState::Running => Self::Running,
            OperationState::Succeeded => Self::Succeeded,
            OperationState::PartiallySucceeded => Self::Partial,
            OperationState::Failed => Self::Failed,
            OperationState::Cancelled => Self::Cancelled,
            OperationState::Interrupted => Self::Interrupted,
        }
    }
}
impl From<ItemState> for OutcomeStatus {
    fn from(state: ItemState) -> Self {
        match state {
            ItemState::Pending => Self::Queued,
            ItemState::Running => Self::Running,
            ItemState::Succeeded => Self::Succeeded,
            ItemState::Skipped => Self::Skipped,
            ItemState::Conflicted => Self::Conflicted,
            ItemState::Failed => Self::Failed,
            ItemState::Cancelled => Self::Cancelled,
        }
    }
}
