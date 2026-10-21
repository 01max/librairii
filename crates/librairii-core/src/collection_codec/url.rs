//! Bounded legacy URL/history handover. No system URL scheme is registered.
//! Reuses the strict frozen wire parser and runtime normalization primitives.
use super::{
    BooleanFilter, ChoiceFilter, Parser, Query, Wire, field, object, query, retire, scalar_integer,
    scalar_string, validate_references,
};
use crate::{
    CollectionId, ShelfId,
    query::{QueryRequest, normalize_request},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("story library hash is invalid")]
pub struct InvalidHash;
type Result<T> = std::result::Result<T, InvalidHash>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scope {
    Library,
    Collection(CollectionId),
    Shelf { id: ShelfId, manual_order: bool },
    Custom,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    pub scope: Scope,
    pub request: QueryRequest,
}

fn percent_decode(input: &str) -> Result<String> {
    let bytes = input.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => result.push(b' '),
            b'%' => {
                let part = bytes.get(i + 1..i + 3).ok_or(InvalidHash)?;
                let hex = std::str::from_utf8(part).map_err(|_| InvalidHash)?;
                result.push(u8::from_str_radix(hex, 16).map_err(|_| InvalidHash)?);
                i += 2;
            }
            b => result.push(b),
        }
        i += 1;
    }
    Ok(String::from_utf8_lossy(&result).into_owned())
}
fn one<'a>(
    values: &'a BTreeMap<String, Vec<String>>,
    key: &str,
    required: bool,
) -> Result<&'a str> {
    match values.get(key) {
        None if !required => Ok(""),
        Some(v) if v.len() == 1 && !v[0].is_empty() => Ok(&v[0]),
        _ => Err(InvalidHash),
    }
}
fn integer(value: &str) -> Result<i64> {
    value.parse().map_err(|_| InvalidHash)
}

fn node(value: Wire, remaining: &mut usize) -> Result<QueryRequest> {
    *remaining = remaining.checked_sub(1).ok_or(InvalidHash)?;
    let mut fields = Vec::new();
    let mut children = Vec::new();
    let mut request = QueryRequest::default();
    for (key, value) in object(value).map_err(|_| InvalidHash)? {
        match field(&key).map_err(|_| InvalidHash)?.as_str() {
            "PAGE" => scalar_integer(value, &mut request.page).map_err(|_| InvalidHash)?,
            "PAGESIZE" => scalar_integer(value, &mut request.page_size).map_err(|_| InvalidHash)?,
            "SORT" => scalar_string(value, &mut request.sort).map_err(|_| InvalidHash)?,
            "ANYOF" => match value {
                Wire::Null => children.clear(),
                Wire::Array(values) => {
                    children.clear();
                    for v in values {
                        children.push(node(v, remaining)?.criteria);
                    }
                }
                _ => return Err(InvalidHash),
            },
            _ => fields.push((key, value)),
        }
    }
    // Validate branch navigation before discarding it from membership data.
    if request.page < 0
        || request.page_size < 0
        || request.page_size > 100
        || !matches!(request.sort.as_str(), "" | "name_asc" | "imported_desc")
    {
        return Err(InvalidHash);
    }
    request.criteria = query(Wire::Object(fields), false).map_err(|_| InvalidHash)?;
    request.criteria.any_of = children;
    Ok(request)
}

fn without_flags(q: &mut Query) {
    q.include_broken = false;
    q.broken_visibility.clear();
    q.broken_state.clear();
    for child in &mut q.any_of {
        without_flags(child);
    }
}
fn canonical(mut request: QueryRequest) -> Result<QueryRequest> {
    request = normalize_request(request).map_err(|_| InvalidHash)?;
    request
        .criteria
        .boolean_filters
        .sort_by_key(|f| f.definition_id);
    request
        .criteria
        .choice_filters
        .sort_by_key(|f| f.definition_id);
    for f in &mut request.criteria.choice_filters {
        f.value_ids.sort();
    }
    Ok(request)
}

pub fn decode(hash: &str, legacy_broken_id: i64) -> Result<QueryRequest> {
    if hash.len() > 262_144 {
        return Err(InvalidHash);
    }
    if hash.is_empty() {
        return canonical(QueryRequest::default());
    }
    let (path, encoded) = hash.split_once('?').ok_or(InvalidHash)?;
    if path != "#/library" {
        return Err(InvalidHash);
    }
    let mut values = BTreeMap::<String, Vec<String>>::new();
    for pair in encoded.split('&').filter(|p| !p.is_empty()) {
        if pair.contains(';') {
            return Err(InvalidHash);
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode(key)?;
        if !matches!(
            key.as_str(),
            "v" | "any"
                | "includeBroken"
                | "brokenVisibility"
                | "brokenState"
                | "name"
                | "page"
                | "size"
                | "sort"
                | "language"
                | "compatibility"
                | "bool"
                | "choice"
        ) {
            return Err(InvalidHash);
        }
        values.entry(key).or_default().push(percent_decode(value)?);
    }
    let version = one(&values, "v", true)?;
    if !matches!(version, "1" | "2" | "3") {
        return Err(InvalidHash);
    }
    let mut request = QueryRequest::default();
    let any = one(&values, "any", false)?;
    if !any.is_empty() {
        let mut parser = Parser {
            bytes: any.as_bytes(),
            offset: 0,
        };
        let wire = parser.value(0).map_err(|_| InvalidHash)?;
        parser.space();
        if parser.offset != any.len() {
            return Err(InvalidHash);
        }
        let mut remaining = 49;
        match wire {
            Wire::Null => (),
            Wire::Array(children) => {
                for child in children {
                    request
                        .criteria
                        .any_of
                        .push(node(child, &mut remaining)?.criteria);
                }
            }
            _ => return Err(InvalidHash),
        }
    }
    let include = one(&values, "includeBroken", false)?;
    let visibility = one(&values, "brokenVisibility", false)?;
    let state = one(&values, "brokenState", false)?;
    if (!include.is_empty() && include != "true")
        || (version == "1" && (!visibility.is_empty() || !state.is_empty()))
        || (version != "1" && !include.is_empty())
    {
        return Err(InvalidHash);
    }
    request.criteria.include_broken = include == "true";
    request.criteria.broken_visibility = visibility.into();
    request.criteria.broken_state = state.into();
    request.criteria.name = one(&values, "name", false)?.into();
    for (key, target) in [
        ("page", &mut request.page),
        ("size", &mut request.page_size),
    ] {
        let v = one(&values, key, false)?;
        if !v.is_empty() {
            *target = integer(v)?;
        }
    }
    request.sort = one(&values, "sort", false)?.into();
    request.criteria.languages = values.get("language").cloned().unwrap_or_default();
    request.criteria.compatibilities = values.get("compatibility").cloned().unwrap_or_default();
    for v in values.get("bool").into_iter().flatten() {
        let (definition, state) = v.split_once(':').ok_or(InvalidHash)?;
        request.criteria.boolean_filters.push(BooleanFilter {
            definition_id: integer(definition)?,
            state: state.into(),
        });
    }
    for v in values.get("choice").into_iter().flatten() {
        let (definition, ids) = v.split_once(':').ok_or(InvalidHash)?;
        request.criteria.choice_filters.push(ChoiceFilter {
            definition_id: integer(definition)?,
            value_ids: ids.split(',').map(integer).collect::<Result<_>>()?,
        });
    }
    validate_references(&request.criteria, &mut 50, false).map_err(|_| InvalidHash)?;
    let mut clean = request.clone();
    without_flags(&mut clean.criteria);
    canonical(clean)?;
    if version != "3" {
        retire(
            &mut request.criteria,
            if version == "1" { 3 } else { 4 },
            legacy_broken_id,
        );
    }
    canonical(request)
}

fn has_membership(q: &Query) -> bool {
    !q.name.is_empty()
        || !q.any_of.is_empty()
        || !q.languages.is_empty()
        || !q.compatibilities.is_empty()
        || !q.boolean_filters.is_empty()
        || !q.choice_filters.is_empty()
}

/// Browser history IDs are considered only when its recorded hash matches.
/// Historical dynamic `shelfId` belongs to Collections, never curated Shelves.
pub fn decode_history(hash: &str, history: &[u8], legacy_broken_id: i64) -> Result<Location> {
    let request = decode(hash, legacy_broken_id)?;
    if history.len() > 262_144 {
        return Err(InvalidHash);
    }
    let state: serde_json::Value = serde_json::from_slice(history).map_err(|_| InvalidHash)?;
    let entry = &state["collectionQuery"];
    let id = |v: &serde_json::Value| v.as_i64().filter(|v| *v > 0 && *v <= 9_007_199_254_740_991);
    let scope = if entry["hash"].as_str() == Some(hash) {
        if entry["group"]["kind"].as_str() == Some("shelf") {
            if let Some(n) = id(&entry["group"]["id"]) {
                Scope::Shelf {
                    id: ShelfId::new(n).map_err(|_| InvalidHash)?,
                    manual_order: entry["group"]["manualOrder"] != false,
                }
            } else {
                fallback(&request.criteria)
            }
        } else if let Some(n) = id(if entry["collectionId"].is_null() {
            &entry["shelfId"]
        } else {
            &entry["collectionId"]
        }) {
            Scope::Collection(CollectionId::new(n).map_err(|_| InvalidHash)?)
        } else {
            match entry["scope"].as_str() {
                Some("all") => Scope::Library,
                Some("custom") => Scope::Custom,
                _ => fallback(&request.criteria),
            }
        }
    } else {
        fallback(&request.criteria)
    };
    Ok(Location { scope, request })
}
fn fallback(q: &Query) -> Scope {
    if has_membership(q) {
        Scope::Custom
    } else {
        Scope::Library
    }
}
