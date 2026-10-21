use super::*;
use librairii_core::{StoryUuid, metadata::trim};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct NormalizedCatalog {
    raw: Arc<[u8]>,
    hash: Checksum,
    locale: Locale,
    records: Vec<CatalogRecord>,
    artworks: Vec<CatalogArtwork>,
}
impl NormalizedCatalog {
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn checksum(&self) -> Checksum {
        self.hash
    }
    pub fn locale(&self) -> &Locale {
        &self.locale
    }
    pub fn records(&self) -> &[CatalogRecord] {
        &self.records
    }
    pub fn artworks(&self) -> &[CatalogArtwork] {
        &self.artworks
    }
}
fn invalid() -> MetadataError {
    MetadataError::InvalidCatalog
}
fn object(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or_else(invalid)
}
fn text(value: Option<&Value>, limit: usize, required: bool) -> Result<String> {
    let out = match value {
        None | Some(Value::Null) => "",
        Some(Value::String(s)) => trim(s),
        _ => return Err(invalid()),
    };
    if out.len() > limit || (required && out.is_empty()) {
        return Err(invalid());
    }
    Ok(out.to_owned())
}
fn attribution(value: Option<&Value>) -> Result<String> {
    let mut names = Vec::new();
    match value {
        None | Some(Value::Null) => {}
        Some(Value::String(s)) => names.push(s.as_str()),
        Some(Value::Array(values)) => {
            for v in values {
                names.push(if v.is_null() {
                    ""
                } else {
                    v.as_str().ok_or_else(invalid)?
                });
            }
        }
        Some(Value::Object(values)) => {
            for (_, v) in values {
                match v {
                    Value::Null => {}
                    Value::Object(person) => {
                        let v = wire(person, "name")?;
                        if let Some(v) = v.filter(|v| !v.is_null()) {
                            names.push(if v.is_null() {
                                ""
                            } else {
                                v.as_str().ok_or_else(invalid)?
                            });
                        }
                    }
                    _ => return Err(invalid()),
                }
            }
        }
        _ => return Err(invalid()),
    }
    let joined = names
        .into_iter()
        .map(trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if joined.len() > 4096 {
        return Err(invalid());
    }
    Ok(joined)
}
fn integer(
    primary: &Map<String, Value>,
    secondary: &Map<String, Value>,
    keys: &[&str],
) -> Result<Option<i64>> {
    for values in [primary, secondary] {
        for key in keys {
            if let Some(v) = values.get(*key).filter(|v| !v.is_null()) {
                return Ok(Some(match v {
                    Value::Number(n) => n.as_i64().ok_or_else(invalid)?,
                    Value::String(s) => trim(s).parse().map_err(|_| invalid())?,
                    _ => return Err(invalid()),
                }));
            }
        }
    }
    Ok(None)
}
fn select<'a>(
    record: &'a Map<String, Value>,
    preferred: &Locale,
) -> Result<Option<(&'a Map<String, Value>, Locale)>> {
    let declared = object(wire(record, "locales_available")?.ok_or_else(invalid)?)?;
    let localized = object(wire(record, "localized_infos")?.ok_or_else(invalid)?)?;
    if declared.is_empty() || localized.is_empty() {
        return Err(invalid());
    }
    let mut available = BTreeMap::new();
    for (key, value) in declared {
        let key = Locale::parse(key)?;
        if available
            .insert(key, value.as_bool().ok_or_else(invalid)?)
            .is_some()
        {
            return Err(invalid());
        }
    }
    let mut seen = BTreeSet::new();
    let mut choices = Vec::new();
    for (key, value) in localized {
        let locale = Locale::parse(key)?;
        if !seen.insert(locale.clone()) {
            return Err(invalid());
        }
        if available.get(&locale) == Some(&true) {
            choices.push((locale, value));
        }
    }
    choices.sort_by_key(|(locale, _)| {
        let rank = if locale == preferred {
            0
        } else if locale.as_str().split('-').next() == preferred.as_str().split('-').next() {
            1
        } else {
            2
        };
        (rank, locale.as_str().to_owned())
    });
    choices
        .into_iter()
        .next()
        .map(|(locale, value)| Ok((object(value)?, locale)))
        .transpose()
}
pub fn normalize(raw: Vec<u8>, requested: &str) -> Result<NormalizedCatalog> {
    if raw.len() > MAX_CATALOG_BYTES {
        return Err(MetadataError::TooLarge);
    }
    let locale = Locale::parse(requested)?;
    let envelope: Value = super::json::parse(&raw)?;
    let response = object(wire(object(&envelope)?, "response")?.ok_or_else(invalid)?)?;
    if response.is_empty() || response.len() > 50_000 {
        return Err(invalid());
    }
    let mut records = Vec::new();
    let mut artwork = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for (source, record) in response {
        if trim(source).is_empty() || source.len() > 1024 {
            return Err(invalid());
        }
        let record = object(record)?;
        let uuid = trim(
            wire(record, "uuid")?
                .and_then(Value::as_str)
                .ok_or_else(invalid)?,
        );
        if uuid.len() != 36 {
            return Err(invalid());
        }
        let uuid: StoryUuid = uuid.parse().map_err(|_| invalid())?;
        if uuid.as_bytes() == &[0; 16] {
            return Err(invalid());
        }
        let Some((localized, language)) = select(record, &locale)? else {
            continue;
        };
        if !seen.insert(uuid) {
            return Err(invalid());
        }
        let title = text(localized.get("title"), 1024, true)?;
        let description = text(localized.get("description"), 128 * 1024, false)?;
        let author = attribution(wire(record, "authors")?)?;
        let mut publisher = attribution(wire(record, "publisher")?)?;
        if publisher.is_empty() {
            publisher = attribution(wire(record, "publishers")?)?;
        }
        let duration_seconds =
            integer(localized, record, &["duration_seconds", "durationSeconds"])?;
        let minimum_age = integer(
            localized,
            record,
            &["minimum_age", "minimumAge", "min_age", "age_min"],
        )?;
        let mut maximum_age = integer(
            localized,
            record,
            &["maximum_age", "maximumAge", "max_age", "age_max"],
        )?;
        if maximum_age == Some(-1) {
            maximum_age = None;
        }
        if duration_seconds.is_some_and(|n| n < 0)
            || minimum_age.is_some_and(|n| n < 0)
            || maximum_age.is_some_and(|n| n < 0)
            || matches!((minimum_age,maximum_age),(Some(a),Some(b))if a>b)
        {
            return Err(invalid());
        }
        let source_updated_at = wire(record, "updated_at")?
            .filter(|v| !v.is_null())
            .map(|v| utc_time(v.as_str().ok_or_else(invalid)?))
            .transpose()?;
        let artwork_id = if let Some(image) = localized.get("image").filter(|v| !v.is_null()) {
            let path = text(wire(object(image)?, "image_url")?, MAX_CATALOG_BYTES, true)?;
            let url = artwork_url(&path)?;
            let id = digest(url.as_bytes());
            artwork.insert(
                id.to_string(),
                CatalogArtwork {
                    id,
                    source_url: url,
                },
            );
            Some(id)
        } else {
            None
        };
        records.push(CatalogRecord {
            uuid,
            title,
            description,
            author,
            publisher,
            language,
            duration_seconds,
            minimum_age,
            maximum_age,
            artwork_id,
            source_record_id: source.clone(),
            source_updated_at,
        });
    }
    if records.is_empty() {
        return Err(invalid());
    }
    Ok(NormalizedCatalog {
        hash: digest(&raw),
        raw: raw.into(),
        locale,
        records,
        artworks: artwork.into_values().collect(),
    })
}

/// JSONv1 folds fixed struct names using Unicode SimpleFold, while localized
/// maps and numeric aliases remain exact. ASCII schema names have only the
/// long-s/Kelvin non-ASCII orbits. Ambiguous duplicate struct spellings reject.
fn wire<'a>(values: &'a Map<String, Value>, field: &str) -> Result<Option<&'a Value>> {
    let mut found = None;
    for (key, value) in values {
        let folded: String = key
            .chars()
            .map(|c| match c {
                '\u{017f}' => 's',
                '\u{212a}' => 'k',
                _ => c.to_ascii_lowercase(),
            })
            .collect();
        if folded == field {
            if found.is_some() {
                return Err(invalid());
            }
            found = Some(value);
        }
    }
    Ok(found)
}
