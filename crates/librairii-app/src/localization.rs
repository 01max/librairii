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
    /// Match the frozen first effective system preference: French when it
    /// resolves to French, English otherwise. Later preferences cannot override
    /// an unsupported preferred language. A saved explicit choice wins separately.
    pub fn from_system<'a>(preferences: impl IntoIterator<Item = &'a str>) -> Self {
        if preferences
            .into_iter()
            .find(|p| !p.trim().is_empty())
            .is_some_and(|p| p.trim().to_ascii_lowercase().starts_with("fr"))
        {
            Self::French
        } else {
            Self::English
        }
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
        f.write_str("settings.invalid")
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
    french_reverse: BTreeMap<String, String>,
}
impl Catalogs {
    pub fn embedded() -> Result<Self, LocalizationError> {
        let english: BTreeMap<String, String> = serde_json::from_str(include_str!(
            "../../../internal/localization/locales/en.json"
        ))
        .map_err(|_| LocalizationError)?;
        let french_entries: OrderedCatalog = serde_json::from_str(include_str!(
            "../../../internal/localization/locales/fr.json"
        ))
        .map_err(|_| LocalizationError)?;
        // The frozen JS Map is insertion ordered and keeps the LAST key for a
        // duplicate translated value. Sorting keys before this projection would
        // change identities such as Annuler (Undo) and Terminé (Done).
        let french_reverse = french_entries
            .0
            .iter()
            .map(|(key, text)| (text.clone(), key.clone()))
            .collect();
        let french: BTreeMap<String, String> = french_entries.0.into_iter().collect();
        if english.is_empty()
            || !english.keys().eq(french.keys())
            || english
                .values()
                .chain(french.values())
                .any(String::is_empty)
        {
            return Err(LocalizationError);
        }
        Ok(Self {
            english,
            french,
            french_reverse,
        })
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
            .or_else(|| self.french_reverse.get(message));
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
            summary: if language == Language::English {
                message.into()
            } else {
                self.text(language, status.key())?.into()
            },
            technical_detail: Some(message),
        })
    }
}

struct OrderedCatalog(Vec<(String, String)>);
impl<'de> Deserialize<'de> for OrderedCatalog {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = OrderedCatalog;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an ordered product catalog")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, String>()? {
                    entries.push(entry);
                }
                Ok(OrderedCatalog(entries))
            }
        }
        decoder.deserialize_map(Visitor)
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
