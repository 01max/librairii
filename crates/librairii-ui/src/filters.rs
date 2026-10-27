//! Owned query editing and labels. No toolkit, database or live-reference authority.
use librairii_app::localization::{Catalogs, Language};
use librairii_core::{
    collection_codec::{self, BooleanFilter, ChoiceFilter, Query},
    query::{QueryInputError, QueryRequest},
    tagging::{CatalogEntry, Kind},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    Name,
    Language(String),
    Compatibility(String),
    Boolean(i64),
    Choice { definition: i64, value: i64 },
    Union,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BooleanState {
    Any,
    Tagged,
    NotTagged,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sort {
    RecentlyAdded,
    Name,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Edit {
    Toggle(Target),
    Boolean {
        definition: i64,
        state: BooleanState,
    },
    Remove(Target),
    Clear,
    ClearTags,
    Sort(Sort),
}
#[derive(Clone, Debug)]
pub struct OptionRow {
    pub target: Target,
    pub label: String,
    /// Any/off=0, tagged/on=1, not-tagged=2.
    pub state: i32,
}
#[derive(Clone, Debug)]
pub struct Active {
    pub target: Target,
    pub label: String,
}

fn facets(
    query: &Query,
    languages: Vec<String>,
    compatibilities: Vec<String>,
) -> Result<Query, QueryInputError> {
    // The frozen codec owns these private fields. Decode only the facet input;
    // do not run the user's name, nested union or tag criteria through an extra
    // saved-query normalization pass while they are being edited.
    let bytes = serde_json::to_vec(
        &serde_json::json!({"languages":languages,"compatibilities":compatibilities}),
    )
    .map_err(|_| QueryInputError::InvalidQuery)?;
    let mut result =
        collection_codec::decode(5, &bytes, 0).map_err(|_| QueryInputError::InvalidQuery)?;
    result.name = query.name.clone();
    result.any_of = query.any_of.clone();
    result.boolean_filters = query.boolean_filters.clone();
    result.choice_filters = query.choice_filters.clone();
    Ok(result)
}
fn toggle(values: &mut Vec<String>, value: String, remove: bool) {
    if values.contains(&value) {
        values.retain(|v| v != &value);
    } else if !remove {
        values.push(value);
    }
}
pub fn edit(request: &QueryRequest, edit: Edit) -> Result<QueryRequest, QueryInputError> {
    let mut result = request.clone();
    result.page = 1;
    let remove = matches!(&edit, Edit::Remove(_));
    match edit {
        Edit::Clear => result.criteria = Query::default(),
        Edit::ClearTags => {
            result.criteria.boolean_filters.clear();
            result.criteria.choice_filters.clear();
        }
        Edit::Sort(sort) => {
            result.sort = match sort {
                Sort::RecentlyAdded => "imported_desc",
                Sort::Name => "name_asc",
            }
            .into()
        }
        Edit::Boolean { definition, state } => {
            result
                .criteria
                .boolean_filters
                .retain(|f| f.definition_id != definition);
            if state != BooleanState::Any {
                result.criteria.boolean_filters.push(BooleanFilter {
                    definition_id: definition,
                    state: if state == BooleanState::Tagged {
                        "true"
                    } else {
                        "false"
                    }
                    .into(),
                });
            }
        }
        Edit::Toggle(target) | Edit::Remove(target) => match target {
            Target::Name => result.criteria.name.clear(),
            Target::Union => result.criteria.any_of.clear(),
            Target::Language(value) => {
                let mut languages = result.criteria.languages().to_vec();
                toggle(&mut languages, value, remove);
                result.criteria = facets(
                    &result.criteria,
                    languages,
                    result.criteria.compatibilities().to_vec(),
                )?;
            }
            Target::Compatibility(value) => {
                let mut compatibilities = result.criteria.compatibilities().to_vec();
                toggle(&mut compatibilities, value, remove);
                result.criteria = facets(
                    &result.criteria,
                    result.criteria.languages().to_vec(),
                    compatibilities,
                )?;
            }
            Target::Boolean(id) => result
                .criteria
                .boolean_filters
                .retain(|f| f.definition_id != id),
            Target::Choice { definition, value } => {
                let mut values = result
                    .criteria
                    .choice_filters
                    .iter()
                    .find(|f| f.definition_id == definition)
                    .map(|f| f.value_ids.clone())
                    .unwrap_or_default();
                if values.contains(&value) {
                    values.retain(|&v| v != value);
                } else if !remove {
                    values.push(value);
                }
                result
                    .criteria
                    .choice_filters
                    .retain(|f| f.definition_id != definition);
                if !values.is_empty() {
                    result.criteria.choice_filters.push(ChoiceFilter {
                        definition_id: definition,
                        value_ids: values,
                    });
                }
            }
        },
    }
    Ok(result)
}
fn translated(catalogs: &Catalogs, language: Language, key: &str) -> String {
    if language == Language::French {
        if key == "Unavailable saved criterion" {
            return "Critère enregistré indisponible".into();
        }
        if key == "Combined query" {
            return "Requête combinée".into();
        }
        if key == "Search" {
            return "Recherche".into();
        }
    }
    catalogs.text(language, key).unwrap_or(key).into()
}
fn definition_label(entry: &CatalogEntry, language: Language, catalogs: &Catalogs) -> String {
    if entry.definition.source == "user" {
        entry.definition.label.clone()
    } else {
        translated(catalogs, language, &entry.definition.label)
    }
}
fn value_label(
    entry: &CatalogEntry,
    value: &str,
    language: Language,
    catalogs: &Catalogs,
) -> String {
    if entry.definition.source == "user" {
        value.into()
    } else {
        translated(catalogs, language, value)
    }
}
fn language_label(value: &str, language: Language, catalogs: &Catalogs) -> String {
    translated(
        catalogs,
        language,
        match value.split('-').next().unwrap_or("") {
            "en" => "English",
            "fr" => "French",
            "de" => "German",
            "es" => "Spanish",
            "it" => "Italian",
            _ => value,
        },
    )
}
fn compatibility_label(value: &str, language: Language, catalogs: &Catalogs) -> String {
    translated(
        catalogs,
        language,
        match value {
            "compatible" => "Compatible",
            "missing" => "Archive missing",
            _ => "Verification failed",
        },
    )
}
pub fn options(
    request: &QueryRequest,
    catalog: &[CatalogEntry],
    languages: &[String],
    language: Language,
    catalogs: &Catalogs,
) -> Vec<OptionRow> {
    let q = &request.criteria;
    let mut locales = languages.to_vec();
    locales.extend_from_slice(q.languages());
    locales.sort();
    locales.dedup();
    let mut rows = locales
        .into_iter()
        .map(|value| OptionRow {
            state: i32::from(q.languages().contains(&value)),
            label: format!(
                "{} · {}",
                translated(catalogs, language, "Language"),
                language_label(&value, language, catalogs)
            ),
            target: Target::Language(value),
        })
        .collect::<Vec<_>>();
    rows.extend(
        ["compatible", "missing", "invalid"]
            .into_iter()
            .map(|value| OptionRow {
                state: i32::from(q.compatibilities().iter().any(|v| v == value)),
                label: format!(
                    "{} · {}",
                    translated(catalogs, language, "Import status"),
                    compatibility_label(value, language, catalogs)
                ),
                target: Target::Compatibility(value.into()),
            }),
    );
    for entry in catalog
        .iter()
        .filter(|e| e.definition.source != "builtin" || e.definition.key != "broken")
    {
        let definition = entry.definition.id;
        let label = definition_label(entry, language, catalogs);
        match entry.definition.kind {
            Kind::Boolean => rows.push(OptionRow {
                target: Target::Boolean(definition),
                label,
                state: q
                    .boolean_filters
                    .iter()
                    .find(|f| f.definition_id == definition)
                    .map_or(0, |f| match f.state.as_str() {
                        "true" => 1,
                        "false" => 2,
                        _ => 0,
                    }),
            }),
            Kind::Choice => {
                for value in &entry.values {
                    rows.push(OptionRow {
                        target: Target::Choice {
                            definition,
                            value: value.id,
                        },
                        label: format!(
                            "{label} · {}",
                            value_label(entry, &value.label, language, catalogs)
                        ),
                        state: i32::from(q.choice_filters.iter().any(|f| {
                            f.definition_id == definition && f.value_ids.contains(&value.id)
                        })),
                    });
                }
            }
        }
    }
    rows
}
pub fn active(
    request: &QueryRequest,
    catalog: &[CatalogEntry],
    language: Language,
    catalogs: &Catalogs,
) -> Vec<Active> {
    let q = &request.criteria;
    let mut result = Vec::new();
    if !q.name.is_empty() {
        result.push(Active {
            target: Target::Name,
            label: format!("{} “{}”", translated(catalogs, language, "Search"), q.name),
        });
    }
    let options = options(request, catalog, &[], language, catalogs);
    for row in options.into_iter().filter(|r| r.state != 0) {
        result.push(Active {
            target: row.target,
            label: if row.state == 2 {
                format!(
                    "{} · {}",
                    translated(catalogs, language, "Not tagged"),
                    row.label
                )
            } else {
                row.label
            },
        });
    }
    for filter in &q.boolean_filters {
        if !catalog
            .iter()
            .any(|e| e.definition.id == filter.definition_id && e.definition.kind == Kind::Boolean)
        {
            result.push(Active {
                target: Target::Boolean(filter.definition_id),
                label: format!(
                    "{} · {}",
                    translated(catalogs, language, "Unavailable saved criterion"),
                    language.number(filter.definition_id)
                ),
            });
        }
    }
    for filter in &q.choice_filters {
        for &value in &filter.value_ids {
            if !catalog.iter().any(|e| {
                e.definition.id == filter.definition_id && e.values.iter().any(|v| v.id == value)
            }) {
                result.push(Active {
                    target: Target::Choice {
                        definition: filter.definition_id,
                        value,
                    },
                    label: format!(
                        "{} · {} / {}",
                        translated(catalogs, language, "Unavailable saved criterion"),
                        language.number(filter.definition_id),
                        language.number(value)
                    ),
                });
            }
        }
    }
    if !q.any_of.is_empty() {
        result.push(Active {
            target: Target::Union,
            label: format!(
                "{} ({})",
                translated(catalogs, language, "Combined query"),
                language.number(q.any_of.len() as i64)
            ),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn facet_edits_preserve_literal_draft_nested_union_and_sort_until_explicit_clear()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut request = QueryRequest {
            sort: "name_asc".into(),
            page: 8,
            page_size: 24,
            ..Default::default()
        };
        request.criteria.name = "  Étoiles %_ ".into();
        let mut branch = Query::default();
        branch.name = "Forêt".into();
        request.criteria.any_of.push(branch);
        let changed = edit(&request, Edit::Toggle(Target::Language("fr".into())))?;
        assert_eq!(changed.criteria.name, request.criteria.name);
        assert_eq!(changed.criteria.any_of, request.criteria.any_of);
        assert_eq!(changed.criteria.languages(), ["fr"]);
        assert_eq!(changed.page, 1);
        assert_eq!(changed.sort, "name_asc");
        let cleared = edit(&changed, Edit::Clear)?;
        assert_eq!(cleared.criteria, Query::default());
        assert_eq!(cleared.sort, "name_asc");
        assert_eq!(cleared.page_size, 24);
        Ok(())
    }
    #[test]
    fn choice_values_are_or_within_definition_and_boolean_removal_preserves_other_groups()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = edit(
            &QueryRequest::default(),
            Edit::Toggle(Target::Choice {
                definition: 3,
                value: 10,
            }),
        )?;
        let second = edit(
            &first,
            Edit::Toggle(Target::Choice {
                definition: 3,
                value: 11,
            }),
        )?;
        let third = edit(
            &second,
            Edit::Boolean {
                definition: 4,
                state: BooleanState::NotTagged,
            },
        )?;
        assert_eq!(third.criteria.choice_filters[0].value_ids, [10, 11]);
        let fourth = edit(
            &third,
            Edit::Remove(Target::Choice {
                definition: 3,
                value: 10,
            }),
        )?;
        assert_eq!(fourth.criteria.choice_filters[0].value_ids, [11]);
        assert_eq!(fourth.criteria.boolean_filters[0].state, "false");
        assert_eq!(edit(&fourth, Edit::ClearTags)?.criteria, Query::default());
        Ok(())
    }
}
