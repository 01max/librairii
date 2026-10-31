// Copyright 2010 The Go Authors. All rights reserved.
// Copyright 2013 The Go Authors. All rights reserved.
// Copyright 2009 The Go Authors. All rights reserved.
// Copyright 2016 The Go Authors. All rights reserved.
// Copyright 2025 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

//! Frozen v1-v5 saved-query codec; no SQL, live-reference or effect authority.
//!
//! PRIVATE DERIVATIVE of the selected Go1.26.5 JSON/query sources. This
//! materialized translation retains the original algorithms and canonical
//! passes. Exact origins and notices are bound by the root query source map;
//! normalization/language derivatives retain their compatibility manifest.

mod compatibility;
pub(crate) mod runtime;
pub mod url;

use self::compatibility::{LanguageError, NormalizeError};
use serde::Serialize;
use std::collections::BTreeSet;

const MAX_BYTES: usize = 262_144;
const MAX_NODES: usize = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CodecError {
    #[error("saved library query is invalid")]
    Invalid,
    #[error("saved library query version is unsupported")]
    UnsupportedVersion,
    #[error("frozen compatibility source invariant failed")]
    FrozenData,
}
type Result<T> = std::result::Result<T, CodecError>;

impl From<NormalizeError> for CodecError {
    fn from(error: NormalizeError) -> Self {
        match error {
            NormalizeError::InputLimit => Self::Invalid,
            NormalizeError::FrozenData => Self::FrozenData,
        }
    }
}
impl From<LanguageError> for CodecError {
    fn from(error: LanguageError) -> Self {
        match error {
            LanguageError::Invalid => Self::Invalid,
            LanguageError::FrozenData => Self::FrozenData,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BooleanFilter {
    pub definition_id: i64,
    pub state: String,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceFilter {
    pub definition_id: i64,
    pub value_ids: Vec<i64>,
}
// Declaration order is the frozen Go Payload marshal order. This
// owned value is never a SQL handle, storage capability or shared app DTO.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    #[serde(skip_serializing_if = "String::is_empty")]
    broken_visibility: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    broken_state: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub any_of: Vec<Query>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    include_broken: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    languages: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    compatibilities: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub boolean_filters: Vec<BooleanFilter>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choice_filters: Vec<ChoiceFilter>,
}

impl Query {
    /// Borrow saved language data without a live-validity or snapshot guarantee.
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// Borrow saved compatibility data without validating the supplied value.
    pub fn compatibilities(&self) -> &[String] {
        &self.compatibilities
    }
}

// The wire tree retains ordered keys and NUMBER LEXEMES. A serde Value
// map would erase duplicate fields, and float coercion would incorrectly
// admit 1.0/1e0 or reject the Go integer spelling -0. Input is bounded before
// allocation. At most50 Payload nodes can be valid (<=104 JSON levels), so
// the128-level parser ceiling cannot reject a valid saved query.
#[derive(Debug)]
enum Wire {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Wire>),
    Object(Vec<(String, Wire)>),
}
struct Parser<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Parser<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.offset)
            .is_some_and(|b| b" \t\r\n".contains(b))
        {
            self.offset += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        if self.bytes.get(self.offset) == Some(&byte) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
    fn hex(&mut self) -> Result<u16> {
        let bytes = self
            .bytes
            .get(self.offset..self.offset + 4)
            .ok_or(CodecError::Invalid)?;
        let mut value = 0;
        for &b in bytes {
            value = value * 16
                + match b {
                    b'0'..=b'9' => u16::from(b - b'0'),
                    b'a'..=b'f' => u16::from(b - b'a' + 10),
                    b'A'..=b'F' => u16::from(b - b'A' + 10),
                    _ => return Err(CodecError::Invalid),
                };
        }
        self.offset += 4;
        Ok(value)
    }
    fn string(&mut self) -> Result<String> {
        if !self.take(b'"') {
            return Err(CodecError::Invalid);
        }
        let mut output = String::new();
        loop {
            let byte = *self.bytes.get(self.offset).ok_or(CodecError::Invalid)?;
            self.offset += 1;
            match byte {
                b'"' => return Ok(output),
                b'\\' => {
                    let escaped = *self.bytes.get(self.offset).ok_or(CodecError::Invalid)?;
                    self.offset += 1;
                    output.push(match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{0008}',
                        b'f' => '\u{000c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let first = self.hex()?;
                            if (0xd800..=0xdbff).contains(&first) {
                                let saved = self.offset;
                                let second = if self.take(b'\\') && self.take(b'u') {
                                    self.hex().ok()
                                } else {
                                    None
                                };
                                if let Some(second) =
                                    second.filter(|c| (0xdc00..=0xdfff).contains(c))
                                {
                                    char::from_u32(
                                        0x10000
                                            + (u32::from(first) - 0xd800) * 1024
                                            + u32::from(second)
                                            - 0xdc00,
                                    )
                                    .ok_or(CodecError::FrozenData)?
                                } else {
                                    self.offset = saved;
                                    '\u{fffd}'
                                }
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                '\u{fffd}'
                            } else {
                                char::from_u32(u32::from(first)).ok_or(CodecError::FrozenData)?
                            }
                        }
                        _ => return Err(CodecError::Invalid),
                    });
                }
                0..=0x1f => return Err(CodecError::Invalid),
                0x20..=0x7f => output.push(char::from(byte)),
                _ => {
                    // Go's unquoteString uses DecodeRune(size1) on EVERY
                    // invalid UTF8 byte; from_utf8_lossy's grouped repair
                    // would change the canonical payload for truncated UTF8.
                    let start = self.offset - 1;
                    let width = match byte {
                        0xc2..=0xdf => 2,
                        0xe0..=0xef => 3,
                        0xf0..=0xf4 => 4,
                        _ => 0,
                    };
                    let valid = (width != 0)
                        .then(|| self.bytes.get(start..start + width))
                        .flatten()
                        .and_then(|bytes| std::str::from_utf8(bytes).ok())
                        .and_then(|text| text.chars().next());
                    if let Some(character) = valid {
                        output.push(character);
                        self.offset = start + width;
                    } else {
                        output.push('\u{fffd}');
                    }
                }
            }
        }
    }
    fn value(&mut self, depth: usize) -> Result<Wire> {
        if depth > 128 {
            return Err(CodecError::Invalid);
        }
        self.space();
        match self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(CodecError::Invalid)?
        {
            b'"' => Ok(Wire::String(self.string()?)),
            b'{' => {
                self.offset += 1;
                self.space();
                let mut fields = Vec::new();
                let mut seen = BTreeSet::new();
                if self.take(b'}') {
                    return Ok(Wire::Object(fields));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    if !seen.insert(compatibility::lowercase_key(&key)?) {
                        return Err(CodecError::Invalid);
                    }
                    self.space();
                    if !self.take(b':') {
                        return Err(CodecError::Invalid);
                    }
                    fields.push((key, self.value(depth + 1)?));
                    self.space();
                    if self.take(b'}') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err(CodecError::Invalid);
                    }
                }
                Ok(Wire::Object(fields))
            }
            b'[' => {
                self.offset += 1;
                self.space();
                let mut values = Vec::new();
                if self.take(b']') {
                    return Ok(Wire::Array(values));
                }
                loop {
                    values.push(self.value(depth + 1)?);
                    self.space();
                    if self.take(b']') {
                        break;
                    }
                    if !self.take(b',') {
                        return Err(CodecError::Invalid);
                    }
                }
                Ok(Wire::Array(values))
            }
            b'-' | b'0'..=b'9' => {
                let start = self.offset;
                while self
                    .bytes
                    .get(self.offset)
                    .is_some_and(|c| b"-+0123456789.eE".contains(c))
                {
                    self.offset += 1;
                }
                let raw = std::str::from_utf8(&self.bytes[start..self.offset])
                    .map_err(|_| CodecError::Invalid)?;
                // JSON syntax is delegated, integer TYPE is not coerced.
                let _: serde_json::Number =
                    serde_json::from_str(raw).map_err(|_| CodecError::Invalid)?;
                Ok(Wire::Number(raw.to_owned()))
            }
            _ => {
                for (literal, value) in [
                    (b"null".as_slice(), Wire::Null),
                    (b"true".as_slice(), Wire::Bool(true)),
                    (b"false".as_slice(), Wire::Bool(false)),
                ] {
                    if self.bytes[self.offset..].starts_with(literal) {
                        self.offset += literal.len();
                        return Ok(value);
                    }
                }
                Err(CodecError::Invalid)
            }
        }
    }
}

fn scalar_string(value: Wire, target: &mut String) -> Result<()> {
    match value {
        Wire::Null => Ok(()),
        Wire::String(s) => {
            *target = s;
            Ok(())
        }
        _ => Err(CodecError::Invalid),
    }
}
fn scalar_integer(value: Wire, target: &mut i64) -> Result<()> {
    match value {
        Wire::Null => Ok(()),
        Wire::Number(raw) => {
            *target = raw.parse().map_err(|_| CodecError::Invalid)?;
            Ok(())
        }
        _ => Err(CodecError::Invalid),
    }
}
fn array<T>(value: Wire, element: impl Fn(Wire) -> Result<T>) -> Result<Vec<T>> {
    match value {
        Wire::Null => Ok(Vec::new()),
        Wire::Array(values) => values.into_iter().map(element).collect(),
        _ => Err(CodecError::Invalid),
    }
}
fn object(value: Wire) -> Result<Vec<(String, Wire)>> {
    match value {
        Wire::Null => Ok(Vec::new()),
        Wire::Object(fields) => Ok(fields),
        _ => Err(CodecError::Invalid),
    }
}
fn field(value: &str) -> Result<String> {
    compatibility::ascii_field_key(value).ok_or(CodecError::Invalid)
}
fn boolean(value: Wire) -> Result<BooleanFilter> {
    let mut result = BooleanFilter::default();
    for (key, value) in object(value)? {
        match field(&key)?.as_str() {
            "DEFINITIONID" => scalar_integer(value, &mut result.definition_id)?,
            "STATE" => scalar_string(value, &mut result.state)?,
            _ => return Err(CodecError::Invalid),
        }
    }
    Ok(result)
}
fn choice(value: Wire) -> Result<ChoiceFilter> {
    let mut result = ChoiceFilter::default();
    for (key, value) in object(value)? {
        match field(&key)?.as_str() {
            "DEFINITIONID" => scalar_integer(value, &mut result.definition_id)?,
            "VALUEIDS" => {
                result.value_ids = array(value, |value| {
                    let mut n = 0;
                    scalar_integer(value, &mut n)?;
                    Ok(n)
                })?
            }
            _ => return Err(CodecError::Invalid),
        }
    }
    Ok(result)
}
fn query(value: Wire, legacy_navigation: bool) -> Result<Query> {
    let mut result = Query::default();
    for (key, value) in object(value)? {
        match field(&key)?.as_str() {
            "BROKENVISIBILITY" => scalar_string(value, &mut result.broken_visibility)?,
            "BROKENSTATE" => scalar_string(value, &mut result.broken_state)?,
            "INCLUDEBROKEN" => match value {
                Wire::Null => (),
                Wire::Bool(b) => result.include_broken = b,
                _ => return Err(CodecError::Invalid),
            },
            "ANYOF" => result.any_of = array(value, |value| query(value, false))?,
            "NAME" => scalar_string(value, &mut result.name)?,
            "LANGUAGES" | "COMPATIBILITIES" => {
                let values = array(value, |value| {
                    let mut s = String::new();
                    scalar_string(value, &mut s)?;
                    Ok(s)
                })?;
                if field(&key)? == "LANGUAGES" {
                    result.languages = values;
                } else {
                    result.compatibilities = values;
                }
            }
            "BOOLEANFILTERS" => result.boolean_filters = array(value, boolean)?,
            "CHOICEFILTERS" => result.choice_filters = array(value, choice)?,
            "PAGE" | "PAGESIZE" if legacy_navigation => {
                scalar_integer(value, &mut 0)?;
            }
            "SORT" if legacy_navigation => {
                scalar_string(value, &mut String::new())?;
            }
            _ => return Err(CodecError::Invalid),
        }
    }
    Ok(result)
}
fn budget(remaining: &mut usize) -> Result<()> {
    *remaining = remaining.checked_sub(1).ok_or(CodecError::Invalid)?;
    Ok(())
}
fn validate_references(q: &Query, remaining: &mut usize, current: bool) -> Result<()> {
    budget(remaining)?;
    if !matches!(
        q.broken_visibility.as_str(),
        "" | "hide" | "include" | "only"
    ) || !matches!(q.broken_state.as_str(), "" | "ignored" | "true" | "false")
        || (current
            && (q.include_broken || !q.broken_visibility.is_empty() || !q.broken_state.is_empty()))
    {
        return Err(CodecError::Invalid);
    }
    let mut ids = BTreeSet::new();
    for f in &q.boolean_filters {
        if f.definition_id <= 0 || !matches!(f.state.as_str(), "ignored" | "true" | "false") {
            return Err(CodecError::Invalid);
        }
        if f.state != "ignored" && !ids.insert(f.definition_id) {
            return Err(CodecError::Invalid);
        }
    }
    for f in &q.choice_filters {
        if f.definition_id <= 0
            || f.value_ids.is_empty()
            || f.value_ids.iter().any(|v| *v <= 0)
            || !ids.insert(f.definition_id)
        {
            return Err(CodecError::Invalid);
        }
    }
    for branch in &q.any_of {
        validate_references(branch, remaining, current)?;
    }
    Ok(())
}
fn retire(q: &mut Query, version: i64, legacy_id: i64) {
    if version < 4 && legacy_id > 0 {
        q.boolean_filters.retain(|f| f.definition_id != legacy_id);
    }
    if version < 5 {
        q.include_broken = false;
        q.broken_visibility.clear();
        q.broken_state.clear();
    }
    for branch in &mut q.any_of {
        retire(branch, version, legacy_id);
    }
}
fn normalize_tree(q: &mut Query, remaining: &mut usize) -> Result<()> {
    budget(remaining)?;
    for branch in &mut q.any_of {
        normalize_tree(branch, remaining)?;
    }
    q.name = compatibility::normalize(&q.name)?;
    let groups = q.boolean_filters.len()
        + q.choice_filters.len()
        + usize::from(!q.languages.is_empty())
        + usize::from(!q.compatibilities.is_empty());
    if q.name.chars().count() > 200
        || groups > 50
        || q.languages.len() > 100
        || q.compatibilities.len() > 3
    {
        return Err(CodecError::Invalid);
    }
    q.languages = q
        .languages
        .iter()
        .map(|v| compatibility::canonical_language(v).map_err(CodecError::from))
        .collect::<Result<_>>()?;
    q.languages.sort();
    q.languages.dedup();
    if q.compatibilities
        .iter()
        .any(|v| !matches!(v.as_str(), "compatible" | "missing" | "invalid"))
    {
        return Err(CodecError::Invalid);
    }
    q.compatibilities.sort();
    q.compatibilities.dedup();
    // referencesFromPayload already validates states and cross-kind IDs.
    // Apply the separate <=100 RAW choice-value limit BEFORE deduplication.
    for f in &mut q.choice_filters {
        if f.value_ids.len() > 100 {
            return Err(CodecError::Invalid);
        }
        let mut seen = BTreeSet::new();
        f.value_ids.retain(|v| seen.insert(*v));
    }
    Ok(())
}
fn canonical_membership(q: &mut Query, remaining: &mut usize) -> Result<()> {
    budget(remaining)?;
    for branch in &mut q.any_of {
        canonical_membership(branch, remaining)?;
    }
    q.boolean_filters.retain(|f| f.state != "ignored");
    // Preserve the source's ORDER of recursive normalization passes. One
    // pass is observably different for compatibility-decomposed capitals.
    let mut remaining = MAX_NODES;
    normalize_tree(q, &mut remaining)?;
    q.boolean_filters.sort_by_key(|f| f.definition_id);
    q.choice_filters.sort_by_key(|f| f.definition_id);
    for f in &mut q.choice_filters {
        f.value_ids.sort();
    }
    Ok(())
}
fn canonical_saved(q: &mut Query) -> Result<()> {
    let mut remaining = MAX_NODES;
    canonical_membership(q, &mut remaining)?;
    for branch in &mut q.any_of {
        canonical_saved(branch)?;
    }
    Ok(())
}
fn decode_payload(version: i64, bytes: &[u8]) -> Result<Query> {
    if !(1..=5).contains(&version) {
        return Err(CodecError::UnsupportedVersion);
    }
    if !(2..=MAX_BYTES).contains(&bytes.len()) {
        return Err(CodecError::Invalid);
    }
    let mut parser = Parser { bytes, offset: 0 };
    let value = parser.value(0)?;
    parser.space();
    if parser.offset != bytes.len() || !matches!(&value, Wire::Object(_)) {
        return Err(CodecError::Invalid);
    }
    let q = query(value, version == 1)?;
    let mut remaining = MAX_NODES;
    validate_references(&q, &mut remaining, version == 5)?;
    Ok(q)
}
/// Reference inspection validates wire shape and tag identities only. It does
/// not normalize membership or grant authority to execute the saved query.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct References {
    pub boolean_filters: Vec<BooleanFilter>,
    pub choice_filters: Vec<ChoiceFilter>,
}
pub fn decode_references(version: i64, bytes: &[u8], legacy_id: i64) -> Result<References> {
    fn collect(q: Query, refs: &mut References, version: i64, legacy: i64) {
        refs.boolean_filters
            .extend(q.boolean_filters.into_iter().filter(|f| {
                f.state != "ignored" && !(version < 4 && legacy > 0 && f.definition_id == legacy)
            }));
        refs.choice_filters.extend(q.choice_filters);
        for branch in q.any_of {
            collect(branch, refs, version, legacy);
        }
    }
    let q = decode_payload(version, bytes)?;
    let mut refs = References::default();
    collect(q, &mut refs, version, legacy_id);
    Ok(refs)
}
pub fn decode(version: i64, bytes: &[u8], legacy_id: i64) -> Result<Query> {
    let mut q = decode_payload(version, bytes)?;
    retire(&mut q, version, legacy_id);
    canonical_saved(&mut q)?;
    Ok(q)
}
pub fn migrate(version: i64, bytes: &[u8], legacy_id: i64) -> Result<(String, Query)> {
    let mut q = decode(version, bytes, legacy_id)?;
    // Migrate calls Decode THEN Encode; Encode performs another canonical
    // saved-query conversion. Do not collapse these observable passes.
    canonical_saved(&mut q)?;
    let mut remaining = MAX_NODES;
    validate_references(&q, &mut remaining, true)?;
    Ok((payload(&q)?, q))
}
/// Encode current membership with the frozen saved-query canonical passes.
/// Navigation and presentation belong to the caller's separate native state.
pub fn encode(mut query: Query) -> Result<String> {
    canonical_saved(&mut query)?;
    let mut remaining = MAX_NODES;
    validate_references(&query, &mut remaining, true)?;
    payload(&query)
}
/// Serialize the supplied owned value in frozen field/escape order and check
/// output bytes only; this does not validate an arbitrary Query value.
pub fn payload(query: &Query) -> Result<String> {
    let payload = serde_json::to_string(query)
        .map_err(|_| CodecError::FrozenData)?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    if payload.len() > MAX_BYTES {
        return Err(CodecError::Invalid);
    }
    Ok(payload)
}

pub(crate) use compatibility::collection_name;
pub(crate) use compatibility::{tag_is_mark, tag_key_decomposition, tag_trim};
pub(crate) fn filename_lower(value: &str) -> Result<String> {
    compatibility::lowercase_key(value).map_err(Into::into)
}

/// Shared frozen locale spelling for official metadata, without live authority.
pub(crate) fn metadata_locale(
    value: &str,
) -> std::result::Result<String, crate::metadata::MetadataError> {
    compatibility::canonical_language(value)
        .map_err(|_| crate::metadata::MetadataError::InvalidCatalog)
}
/// Catalog titles use the same one-pass frozen search text normalization.
pub(crate) fn metadata_text(
    value: &str,
) -> std::result::Result<String, crate::metadata::MetadataError> {
    compatibility::normalize_runtime(value)
        .map_err(|_| crate::metadata::MetadataError::InvalidCatalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated(version: i64, input: &[u8], legacy: i64) -> Result<String> {
        migrate(version, input, legacy).map(|(payload, _)| payload)
    }
    #[test]
    fn wire_keys_nulls_utf8_surrogates_and_integer_lexemes_are_go_compatible() -> Result<()> {
        assert_eq!(
            migrated(5, br#"{"NAME":"A","name":null}"#, 0),
            Err(CodecError::Invalid)
        );
        assert_eq!(
            migrated(1, br#"{"name":"A","page":-0}"#, 0)?,
            r#"{"name":"a"}"#
        );
        assert_eq!(migrated(1, br#"{"page":1.0}"#, 0), Err(CodecError::Invalid));
        assert_eq!(migrated(1, br#"{"page":1e0}"#, 0), Err(CodecError::Invalid));
        assert_eq!(
            migrated(5, br#"{"name":"\ud800X\udc00"}"#, 0)?,
            "{\"name\":\"�x�\"}"
        );
        assert_eq!(
            migrated(5, b"{\"name\":\"\xe2\x82\"}", 0)?,
            "{\"name\":\"��\"}"
        );
        assert_eq!(
            migrated(
                5,
                "{\"languageſ\":[\"en\"],\"languages\":null}".as_bytes(),
                0
            )?,
            "{}"
        );
        assert_eq!(
            migrated(5, br#"{"boolean\uFB01lters":[]}"#, 0),
            Err(CodecError::Invalid)
        );
        assert_eq!(
            migrated(
                5,
                "{\"booleanFilters\":[{\"definitionId\":1,\"ſtate\":\"true\",\"state\":null}]}"
                    .as_bytes(),
                0
            )?,
            r#"{"booleanFilters":[{"definitionId":1,"state":"true"}]}"#
        );
        Ok(())
    }
    #[test]
    fn strict_versions_references_and_retired_broken_order_prevent_broadening() -> Result<()> {
        let raw = r#"{"name":" ÉTÉ ","booleanFilters":[{"definitionId":7,"state":"true"},{"definitionId":8,"state":"false"}],"anyOf":[{"booleanFilters":[{"definitionId":7,"state":"ignored"}]}]}"#.as_bytes();
        assert_eq!(
            migrated(3, raw, 7)?,
            r#"{"anyOf":[{}],"name":"ete","booleanFilters":[{"definitionId":8,"state":"false"}]}"#
        );
        assert!(migrated(4, raw, 7)?.contains("\"definitionId\":7"));
        for version in 1..=5 {
            assert_eq!(migrated(version, br#"{"booleanFilters":[{"definitionId":7,"state":"true"},{"definitionId":7,"state":"false"}]}"#, 7), Err(CodecError::Invalid));
        }
        assert_eq!(
            migrated(6, b"invalid", 7),
            Err(CodecError::UnsupportedVersion)
        );
        assert_eq!(
            migrated(5, br#"{"includeBroken":true}"#, 0),
            Err(CodecError::Invalid)
        );
        assert_eq!(migrated(4, br#"{"includeBroken":true}"#, 0)?, "{}");
        assert_eq!(
            migrated(1, br#"{"anyOf":[{"page":1}]}"#, 0),
            Err(CodecError::Invalid)
        );
        assert_eq!(migrated(5, br#"{"booleanFilters":[{"definitionId":1,"state":"ignored"}],"choiceFilters":[{"definitionId":1,"valueIds":[2,2]}]}"#, 0)?, r#"{"choiceFilters":[{"definitionId":1,"valueIds":[2]}]}"#);
        Ok(())
    }
    #[test]
    fn canonical_wire_order_budgets_and_repeated_normalization_match_source() -> Result<()> {
        let input = "{\"name\":\"𝑨 <>&\u{2028}\",\"languages\":[\"fr_fr\",\"en-GB\",\"fr-FR\"],\"compatibilities\":[\"missing\",\"compatible\",\"missing\"],\"choiceFilters\":[{\"definitionId\":5,\"valueIds\":[7,2,7]}]}";
        assert_eq!(
            migrated(5, input.as_bytes(), 0)?,
            r#"{"name":"a \u003c\u003e\u0026","languages":["en-GB","fr-FR"],"compatibilities":["compatible","missing"],"choiceFilters":[{"definitionId":5,"valueIds":[2,7]}]}"#
        );
        let branches = |n| format!("{{\"anyOf\":[{}]}}", vec!["null"; n].join(","));
        assert!(migrated(5, branches(49).as_bytes(), 0).is_ok());
        assert_eq!(
            migrated(5, branches(50).as_bytes(), 0),
            Err(CodecError::Invalid)
        );
        let values = format!(
            "{{\"choiceFilters\":[{{\"definitionId\":1,\"valueIds\":[{}]}}]}}",
            vec!["2"; 101].join(",")
        );
        assert_eq!(migrated(5, values.as_bytes(), 0), Err(CodecError::Invalid));
        assert_eq!(
            migrated(5, &[b' '; MAX_BYTES + 1], 0),
            Err(CodecError::Invalid)
        );
        for input in [
            b"null".as_slice(),
            b"{} {}",
            b"[]",
            b"{\"name\":true}",
            b"{\"unknown\":null}",
            b"{\"anyOf\":[null,]}",
        ] {
            assert_eq!(migrated(5, input, 0), Err(CodecError::Invalid), "{input:?}");
        }
        Ok(())
    }

    #[test]
    fn strict_codec_decode_and_migrate_match_actual_frozen_go_cases()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Case {
            name: String,
            version: i64,
            legacy_id: i64,
            bytes: String,
            decoded_payload: String,
            decode_error: String,
            migrated_payload: String,
            migrate_error: String,
        }
        #[derive(serde::Deserialize)]
        struct Reference {
            cases: Vec<Case>,
            provenance: serde_json::Value,
        }
        let reference: Reference =
            serde_json::from_str(include_str!("compatibility/codec-reference.json"))?;
        assert_eq!(
            reference.provenance["oracle"],
            "73a93303a9a44cf0a92eaf525331561ed8c04460"
        );
        assert_eq!(reference.provenance["originalFiles"], 436);
        assert_eq!(reference.provenance["unchangedOriginalFiles"], 436);
        assert_eq!(reference.provenance["unchangedZipMembers"], 487);
        assert_eq!(reference.cases.len(), 2864);
        let check = |result: Result<String>,
                     expected: &str,
                     code: &str,
                     c: &Case,
                     phase|
         -> std::result::Result<(), Box<dyn std::error::Error>> {
            match code {
                "" => assert_eq!(
                    result.as_deref(),
                    Ok(expected),
                    "{} v{} legacy{} {phase}",
                    c.name,
                    c.version,
                    c.legacy_id
                ),
                "invalid" => assert_eq!(
                    result,
                    Err(CodecError::Invalid),
                    "{} v{} legacy{} {phase}",
                    c.name,
                    c.version,
                    c.legacy_id
                ),
                "unsupported" => assert_eq!(
                    result,
                    Err(CodecError::UnsupportedVersion),
                    "{} {phase}",
                    c.name
                ),
                _ => {
                    return Err(std::io::Error::other(
                        "unrecognized actual Go error classification",
                    )
                    .into());
                }
            }
            Ok(())
        };
        for c in reference.cases {
            let bytes = c
                .bytes
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair)?, 16).map_err(Into::into))
                .collect::<std::result::Result<Vec<_>, Box<dyn std::error::Error>>>()?;
            let decoded = decode(c.version, &bytes, c.legacy_id).and_then(|q| payload(&q));
            check(decoded, &c.decoded_payload, &c.decode_error, &c, "decode")?;
            check(
                migrated(c.version, &bytes, c.legacy_id),
                &c.migrated_payload,
                &c.migrate_error,
                &c,
                "migrate",
            )?;
        }
        Ok(())
    }
}
