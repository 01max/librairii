//! Runtime request policy from frozen story_query.go/query.go, not SQL results.
//! Mutable decoded criteria and serialized payloads carry no validity authority.
use librairii_core::{
    collection_codec::{self, BooleanFilter, ChoiceFilter, CodecError, Query},
    query::{QueryInputError, QueryRequest, normalize_request},
};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

fn named(value: &str) -> Query {
    let mut query = Query::default();
    query.name = value.to_owned();
    query
}

fn request(criteria: Query) -> QueryRequest {
    QueryRequest {
        criteria,
        ..QueryRequest::default()
    }
}

fn boolean(definition_id: i64, state: &str) -> BooleanFilter {
    BooleanFilter {
        definition_id,
        state: state.to_owned(),
    }
}

fn choice(definition_id: i64, values: &[i64]) -> ChoiceFilter {
    ChoiceFilter {
        definition_id,
        value_ids: values.to_vec(),
    }
}

fn rejects(input: QueryRequest) {
    assert!(matches!(
        normalize_request(input),
        Err(QueryInputError::InvalidQuery)
    ));
}

#[test]
fn query_runtime_navigation_defaults_and_exact_explicit_values_are_validated() -> TestResult {
    let normalized = normalize_request(QueryRequest::default())?;
    assert_eq!(normalized.page, 1);
    assert_eq!(normalized.page_size, 24);
    assert_eq!(normalized.sort, "name_asc");

    for page_size in [1, 100] {
        for sort in ["name_asc", "imported_desc"] {
            let input = QueryRequest {
                criteria: Query::default(),
                page: i64::MAX,
                page_size,
                sort: sort.to_owned(),
            };
            // Normalization validates navigation, without inventing an offset
            // calculation, row count, picker cap, or Count SQL result.
            assert_eq!(normalize_request(input.clone())?, input);
        }
    }
    rejects(QueryRequest {
        page: -1,
        ..QueryRequest::default()
    });
    for page_size in [-1, 101, i64::MAX] {
        rejects(QueryRequest {
            page_size,
            ..QueryRequest::default()
        });
    }
    for sort in ["name_desc", "Name_asc", " name_asc", "imported_asc"] {
        rejects(QueryRequest {
            sort: sort.to_owned(),
            ..QueryRequest::default()
        });
    }
    Ok(())
}

#[test]
fn query_runtime_root_and_each_child_receive_exactly_one_unicode_normalization_pass() -> TestResult
{
    let mut criteria = named("𝑨");
    let mut first = named("𝑩");
    first.any_of.push(named("𝑪"));
    criteria.any_of = vec![first, named(" ÉTÉ ")];
    let once = normalize_request(request(criteria))?;
    // Full fold precedes decomposition. These mathematical capitals become
    // ASCII capitals after ONE pass; saved-query repeated passes differ.
    assert_eq!(once.criteria.name, "A");
    assert_eq!(once.criteria.any_of[0].name, "B");
    assert_eq!(once.criteria.any_of[0].any_of[0].name, "C");
    assert_eq!(once.criteria.any_of[1].name, "ete");
    let twice = normalize_request(once)?;
    assert_eq!(twice.criteria.name, "a");
    assert_eq!(twice.criteria.any_of[0].name, "b");
    assert_eq!(twice.criteria.any_of[0].any_of[0].name, "c");
    assert_eq!(twice.criteria.any_of[1].name, "ete");
    Ok(())
}

#[test]
fn query_runtime_retains_ignored_filters_definition_order_and_stable_choice_deduplication()
-> TestResult {
    let mut criteria = Query::default();
    criteria.boolean_filters = vec![
        boolean(9, "ignored"),
        boolean(2, "true"),
        boolean(7, "false"),
    ];
    criteria.choice_filters = vec![choice(12, &[9, 2, 9, 7, 2]), choice(3, &[6, 1])];
    let original_booleans = criteria.boolean_filters.clone();
    let normalized = normalize_request(request(criteria))?;
    assert_eq!(normalized.criteria.boolean_filters, original_booleans);
    assert_eq!(
        normalized.criteria.choice_filters,
        vec![choice(12, &[9, 2, 7]), choice(3, &[6, 1])]
    );
    Ok(())
}

#[test]
fn query_runtime_definition_duplicates_include_ignored_and_cross_kind_filters() -> TestResult {
    for filters in [
        vec![boolean(7, "ignored"), boolean(7, "true")],
        vec![boolean(7, "ignored"), boolean(7, "ignored")],
        vec![boolean(7, "true"), boolean(7, "false")],
    ] {
        let mut criteria = Query::default();
        criteria.boolean_filters = filters;
        rejects(request(criteria));
    }
    let mut cross_kind = Query::default();
    cross_kind.boolean_filters.push(boolean(7, "ignored"));
    cross_kind.choice_filters.push(choice(7, &[1]));
    rejects(request(cross_kind));
    let mut duplicate_choices = Query::default();
    duplicate_choices.choice_filters = vec![choice(7, &[1]), choice(7, &[2])];
    rejects(request(duplicate_choices));

    for definition_id in [0, -1] {
        let mut criteria = Query::default();
        criteria
            .boolean_filters
            .push(boolean(definition_id, "true"));
        rejects(request(criteria));
        let mut criteria = Query::default();
        criteria.choice_filters.push(choice(definition_id, &[1]));
        rejects(request(criteria));
    }
    for state in ["", "TRUE", " false ", "unknown"] {
        let mut criteria = Query::default();
        criteria.boolean_filters.push(boolean(1, state));
        rejects(request(criteria));
    }
    let mut positive_without_database_claim = Query::default();
    positive_without_database_claim
        .boolean_filters
        .push(boolean(i64::MAX, "ignored"));
    assert_eq!(
        normalize_request(request(positive_without_database_claim))?
            .criteria
            .boolean_filters,
        vec![boolean(i64::MAX, "ignored")]
    );
    Ok(())
}

#[test]
fn query_runtime_node_budget_counts_root_flat_branches_and_nested_descendants() -> TestResult {
    let mut flat = Query::default();
    flat.any_of = vec![Query::default(); 49];
    assert_eq!(
        normalize_request(request(flat.clone()))?
            .criteria
            .any_of
            .len(),
        49
    );
    flat.any_of.push(Query::default());
    rejects(request(flat));

    let chain = |nodes| {
        let mut current = Query::default();
        for _ in 1..nodes {
            let mut parent = Query::default();
            parent.any_of.push(current);
            current = parent;
        }
        current
    };
    normalize_request(request(chain(50)))?;
    rejects(request(chain(51)));
    let mut invalid_child = Query::default();
    invalid_child.any_of.push(named(&"漢".repeat(201)));
    rejects(request(invalid_child));
    Ok(())
}

#[test]
fn query_runtime_name_limit_counts_normalized_unicode_characters() -> TestResult {
    for raw in ["É".repeat(200), "漢".repeat(200)] {
        assert_eq!(
            normalize_request(request(named(&raw)))?
                .criteria
                .name
                .chars()
                .count(),
            200
        );
    }
    for raw in ["É".repeat(201), "漢".repeat(201)] {
        rejects(request(named(&raw)));
    }
    let decomposed = "a\u{0301}".repeat(200);
    assert_eq!(
        normalize_request(request(named(&decomposed)))?
            .criteria
            .name,
        "a".repeat(200)
    );
    Ok(())
}

#[test]
fn query_runtime_large_removable_names_do_not_inherit_saved_wire_byte_refusal() -> TestResult {
    for raw in [" ".repeat(262_145), "\u{0301}".repeat(131_073)] {
        assert!(raw.len() > 262_144);
        let saved_wire = format!("{{\"name\":\"{raw}\"}}");
        assert_eq!(
            collection_codec::decode(5, saved_wire.as_bytes(), 0),
            Err(CodecError::Invalid)
        );
        assert_eq!(normalize_request(request(named(&raw)))?.criteria.name, "");
    }
    Ok(())
}

#[test]
fn query_runtime_group_limit_includes_language_and_compatibility_presence() -> TestResult {
    // Decode only seeds inaccessible lists. Its saved normalization means this
    // cannot prove runtime handling of RAW list cardinality/aliases/duplicates.
    let seed = collection_codec::decode(
        5,
        br#"{"languages":["fr_fr","en"],"compatibilities":["missing","compatible"]}"#,
        0,
    )?;
    assert_eq!(
        seed.languages()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["en", "fr-FR"]
    );
    assert_eq!(
        seed.compatibilities()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["compatible", "missing"]
    );
    let mut boundary = seed.clone();
    boundary.boolean_filters = (1..=48).map(|id| boolean(id, "ignored")).collect();
    let valid = normalize_request(request(boundary.clone()))?;
    assert_eq!(valid.criteria.boolean_filters.len(), 48);
    assert_eq!(valid.criteria.languages(), seed.languages());
    assert_eq!(valid.criteria.compatibilities(), seed.compatibilities());
    boundary.boolean_filters.push(boolean(49, "true"));
    rejects(request(boundary));

    let mut filters_only = Query::default();
    filters_only.boolean_filters = (1..=50).map(|id| boolean(id, "ignored")).collect();
    assert_eq!(
        normalize_request(request(filters_only.clone()))?
            .criteria
            .boolean_filters
            .len(),
        50
    );
    filters_only.boolean_filters.push(boolean(51, "ignored"));
    rejects(request(filters_only));
    Ok(())
}

#[test]
fn query_runtime_choice_cardinality_is_checked_before_stable_deduplication() -> TestResult {
    let mut boundary = Query::default();
    boundary.choice_filters.push(choice(1, &[7; 100]));
    assert_eq!(
        normalize_request(request(boundary))?
            .criteria
            .choice_filters,
        vec![choice(1, &[7])]
    );
    for values in [vec![], vec![7; 101], vec![0], vec![-1], vec![7, 0, 7]] {
        let mut criteria = Query::default();
        criteria.choice_filters.push(choice(1, &values));
        rejects(request(criteria));
    }
    let mut positive_without_database_claim = Query::default();
    positive_without_database_claim
        .choice_filters
        .push(choice(i64::MAX, &[i64::MAX, 1, i64::MAX]));
    assert_eq!(
        normalize_request(request(positive_without_database_claim))?
            .criteria
            .choice_filters,
        vec![choice(i64::MAX, &[i64::MAX, 1])]
    );
    Ok(())
}

#[test]
fn query_runtime_mutated_decoded_value_and_payload_do_not_certify_runtime_validity() -> TestResult {
    let mut criteria = collection_codec::decode(5, b"{}", 0)?;
    criteria.boolean_filters.push(boolean(0, "true"));
    // payload checks serialization/output bytes, so a serialized invalid
    // mutable value must still be rejected at the runtime request boundary.
    let serialized = collection_codec::payload(&criteria)?;
    assert!(serialized.contains("\"definitionId\":0"));
    rejects(request(criteria));

    let mut invalid_navigation = request(collection_codec::decode(5, b"{}", 0)?);
    invalid_navigation.page_size = 101;
    rejects(invalid_navigation);
    Ok(())
}
