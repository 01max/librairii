//! The selected frozen story-library runtime pass on the existing owned Query.
//! This shares frozen text/language primitives, not saved codec canonical passes.

use super::{Query, compatibility};
use crate::query::{QueryInputError, QueryRequest};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, QueryInputError>;

pub(crate) fn normalize_request(mut request: QueryRequest) -> Result<QueryRequest> {
    let mut remaining = 50;
    normalize_node(
        &mut request.criteria,
        &mut remaining,
        Some((&mut request.page, &mut request.page_size, &mut request.sort)),
    )?;
    Ok(request)
}

fn normalize_navigation(page: &mut i64, page_size: &mut i64, sort: &mut String) -> Result<()> {
    if *page == 0 {
        *page = 1;
    }
    if *page_size == 0 {
        *page_size = 24;
    }
    if sort.is_empty() {
        *sort = "name_asc".to_owned();
    }
    if *page < 1
        || !(1..=100).contains(page_size)
        || !matches!(sort.as_str(), "name_asc" | "imported_desc")
    {
        return Err(QueryInputError::InvalidQuery);
    }
    Ok(())
}

fn normalize_node(
    query: &mut Query,
    remaining: &mut usize,
    navigation: Option<(&mut i64, &mut i64, &mut String)>,
) -> Result<()> {
    // Frozen runtime rejects these compatibility-only values before consuming
    // its node budget. It never performs the saved codec's retirement pass.
    if query.include_broken || !query.broken_visibility.is_empty() || !query.broken_state.is_empty()
    {
        return Err(QueryInputError::InvalidQuery);
    }
    *remaining = remaining
        .checked_sub(1)
        .ok_or(QueryInputError::InvalidQuery)?;
    for branch in &mut query.any_of {
        normalize_node(branch, remaining, None)?;
    }
    // Query has no nested navigation fields; their source defaults are valid.
    // Only the root's actual request navigation can supply nondefault values.
    if let Some((page, page_size, sort)) = navigation {
        normalize_navigation(page, page_size, sort)?;
    }
    query.name = compatibility::normalize_runtime(&query.name).map_err(|error| match error {
        compatibility::RuntimeNormalizeError::FrozenData => QueryInputError::FrozenData,
        compatibility::RuntimeNormalizeError::ResourceOverflow => QueryInputError::ResourceOverflow,
    })?;
    let groups = query
        .boolean_filters
        .len()
        .checked_add(query.choice_filters.len())
        .and_then(|n| n.checked_add(usize::from(!query.languages.is_empty())))
        .and_then(|n| n.checked_add(usize::from(!query.compatibilities.is_empty())))
        .ok_or(QueryInputError::ResourceOverflow)?;
    if query.name.chars().count() > 200
        || groups > 50
        || query.languages.len() > 100
        || query.compatibilities.len() > 3
    {
        return Err(QueryInputError::InvalidQuery);
    }
    let mut languages = BTreeSet::new();
    for language in &query.languages {
        languages.insert(compatibility::canonical_language(language).map_err(
            |error| match error {
                compatibility::LanguageError::Invalid => QueryInputError::InvalidQuery,
                compatibility::LanguageError::FrozenData => QueryInputError::FrozenData,
            },
        )?);
    }
    query.languages = languages.into_iter().collect();
    let mut compatibilities = BTreeSet::new();
    for value in &query.compatibilities {
        if !matches!(value.as_str(), "compatible" | "missing" | "invalid") {
            return Err(QueryInputError::InvalidQuery);
        }
        compatibilities.insert(value.clone());
    }
    query.compatibilities = compatibilities.into_iter().collect();

    // Runtime counts ignored Boolean definitions too. Saved-query reference
    // validation/omission is deliberately not used here.
    let mut definitions = BTreeSet::new();
    for filter in &query.boolean_filters {
        if filter.definition_id <= 0
            || !matches!(filter.state.as_str(), "ignored" | "true" | "false")
            || !definitions.insert(filter.definition_id)
        {
            return Err(QueryInputError::InvalidQuery);
        }
    }
    for filter in &mut query.choice_filters {
        if filter.definition_id <= 0
            || filter.value_ids.is_empty()
            || filter.value_ids.len() > 100
            || !definitions.insert(filter.definition_id)
        {
            return Err(QueryInputError::InvalidQuery);
        }
        let mut seen = BTreeSet::new();
        let mut values = Vec::new();
        for &value in &filter.value_ids {
            if value <= 0 {
                return Err(QueryInputError::InvalidQuery);
            }
            if seen.insert(value) {
                values.push(value);
            }
        }
        filter.value_ids = values;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_private_lists_use_frozen_language_and_per_node_source_limits() -> Result<()> {
        let child = Query {
            languages: vec![" fr_fr ".to_owned(), "en-GB".to_owned(), "fr-FR".to_owned()],
            compatibilities: vec![
                "missing".to_owned(),
                "compatible".to_owned(),
                "missing".to_owned(),
            ],
            ..Query::default()
        };
        let mut request = QueryRequest::default();
        request.criteria.any_of.push(child);
        let normalized = normalize_request(request)?;
        let child = &normalized.criteria.any_of[0];
        assert_eq!(child.languages(), &["en-GB", "fr-FR"]);
        assert_eq!(child.compatibilities(), &["compatible", "missing"]);

        for (languages, compatibilities) in [
            (vec!["en-GB".to_owned(); 101], Vec::new()),
            (Vec::new(), vec!["missing".to_owned(); 4]),
            (vec!["und".to_owned()], Vec::new()),
            (Vec::new(), vec!["valid".to_owned()]),
        ] {
            let mut request = QueryRequest::default();
            request.criteria.languages = languages;
            request.criteria.compatibilities = compatibilities;
            assert_eq!(
                normalize_request(request),
                Err(QueryInputError::InvalidQuery)
            );
        }
        Ok(())
    }

    #[test]
    fn runtime_rejects_private_retired_flags_without_retiring_or_saving() {
        for field in 0..3 {
            let mut request = QueryRequest::default();
            match field {
                0 => request.criteria.include_broken = true,
                1 => request.criteria.broken_visibility = "hide".to_owned(),
                _ => request.criteria.broken_state = "ignored".to_owned(),
            }
            assert_eq!(
                normalize_request(request),
                Err(QueryInputError::InvalidQuery)
            );
        }
    }

    #[test]
    fn runtime_visits_children_then_navigation_then_node_fields_in_source_order() {
        let mut query = Query {
            name: "𝑨".to_owned(),
            ..Query::default()
        };
        let child = Query {
            name: "𝑩".to_owned(),
            ..Query::default()
        };
        query.any_of.push(child);
        let (mut page, mut size, mut sort) = (-1, 0, String::new());
        assert_eq!(
            normalize_node(&mut query, &mut 50, Some((&mut page, &mut size, &mut sort))),
            Err(QueryInputError::InvalidQuery)
        );
        assert_eq!(query.any_of[0].name, "B");
        assert_eq!(query.name, "𝑨");

        query.boolean_filters.push(super::super::BooleanFilter {
            definition_id: 0,
            state: "true".to_owned(),
        });
        assert_eq!(
            normalize_node(&mut query, &mut 50, None),
            Err(QueryInputError::InvalidQuery)
        );
        assert_eq!(query.name, "A");
        assert_eq!(query.any_of[0].name, "b");
    }
}
