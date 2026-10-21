use librairii_core::collection_codec::url::{self, Scope};
use serde_json::{Value, json};
use std::error::Error;
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn url_shared_current_and_legacy_fixtures_preserve_non_broken_criteria() -> TestResult {
    let fixtures: Vec<Value> = serde_json::from_str(include_str!(
        "../../../internal/library/testdata/story_library_query_codec.json"
    ))?;
    for fixture in fixtures {
        let hash = fixture["hash"].as_str().ok_or("missing hash")?;
        let expected = &fixture["query"];
        let actual = url::decode(hash, 0)?;
        assert_eq!(
            actual.criteria.name,
            expected["name"].as_str().ok_or("name")?
        );
        assert_eq!(actual.page, expected["page"].as_i64().ok_or("page")?);
        assert_eq!(
            actual.page_size,
            expected["pageSize"].as_i64().ok_or("size")?
        );
        assert_eq!(actual.sort, expected["sort"].as_str().ok_or("sort")?);
        assert_eq!(
            serde_json::to_value(actual.criteria.languages())?,
            expected["languages"]
        );
        assert_eq!(
            serde_json::to_value(actual.criteria.compatibilities())?,
            expected["compatibilities"]
        );
        assert_eq!(
            serde_json::to_value(&actual.criteria.boolean_filters)?,
            expected["booleanFilters"]
        );
        assert_eq!(
            serde_json::to_value(&actual.criteria.choice_filters)?,
            expected["choiceFilters"]
        );
    }
    Ok(())
}

#[test]
fn legacy_flags_retire_recursively_while_current_tag_reuse_survives() -> TestResult {
    let old = url::decode(
        "#/library?v=1&bool=7%3Atrue&any=%5B%7B%22booleanFilters%22%3A%5B%7B%22definitionId%22%3A7%2C%22state%22%3A%22false%22%7D%5D%2C%22name%22%3A%22Moon%22%7D%5D",
        7,
    )?;
    assert!(old.criteria.boolean_filters.is_empty());
    assert!(old.criteria.any_of[0].boolean_filters.is_empty());
    assert_eq!(old.criteria.any_of[0].name, "moon");
    let current = url::decode("#/library?v=3&bool=7%3Atrue", 7)?;
    assert_eq!(current.criteria.boolean_filters[0].definition_id, 7);
    for hash in [
        "#/library?v=3&brokenVisibility=hide",
        "#/library?v=3&name=one&name=two",
        "#/library?v=99",
        "#/library?v=3&unknown=a",
        "#/library?v=3&name=%Q0",
        "#/library?v=3&bool=7%3Atrue&bool=7%3Afalse",
        "#/library?v=1&bool=7%3Atrue&bool=7%3Afalse",
        "#/library?v=3&page=-1",
        "#/library?v=3&any=%5B%7B%22page%22%3A-1%7D%5D",
    ] {
        assert!(url::decode(hash, 7).is_err(), "{hash}");
    }
    Ok(())
}

#[test]
fn history_hash_binding_and_typed_ids_distinguish_dynamic_and_curated_shelves() -> TestResult {
    let hash = "#/library?v=3";
    let legacy = url::decode_history(
        hash,
        &serde_json::to_vec(&json!({"collectionQuery":{"hash":hash,"shelfId":7}}))?,
        0,
    )?;
    let curated = url::decode_history(
        hash,
        &serde_json::to_vec(
            &json!({"collectionQuery":{"hash":hash,"collectionId":7,"group":{"kind":"shelf","id":7,"manualOrder":false}}}),
        )?,
        0,
    )?;
    assert!(matches!(legacy.scope, Scope::Collection(_)));
    assert!(matches!(
        curated.scope,
        Scope::Shelf {
            manual_order: false,
            ..
        }
    ));
    assert_ne!(legacy.scope, curated.scope);
    assert_eq!(
        url::decode_history(
            hash,
            br##"{"collectionQuery":{"hash":"#/other","shelfId":7}}"##,
            0
        )?
        .scope,
        Scope::Library
    );
    assert_eq!(
        url::decode_history("#/library?v=3&name=moon", b"null", 0)?.scope,
        Scope::Custom
    );
    Ok(())
}

#[test]
fn url_bounds_and_non_idempotent_unicode_are_validated_without_extra_output_passes() -> TestResult {
    assert_eq!(
        url::decode("#/library?v=3&name=%F0%9D%91%A8", 0)?
            .criteria
            .name,
        "A"
    );
    assert_eq!(
        url::decode("#/library?v=3&name=100%25+pure", 0)?
            .criteria
            .name,
        "100% pure"
    );
    assert!(url::decode(&format!("#/library?v=3&name={}", "x".repeat(262_144)), 0).is_err());
    let child = serde_json::to_string(&vec![json!({}); 50])?;
    assert!(url::decode(&format!("#/library?v=3&any={child}"), 0).is_err());
    Ok(())
}

#[test]
fn percent_escapes_require_two_hex_digits_and_never_turn_malformed_input_into_empty_success() {
    for encoded in ["%+A", "%+0", "% A", "%-1", "%G0", "%0G", "%0", "%"] {
        assert!(
            url::decode(&format!("#/library?v=3&name={encoded}"), 0).is_err(),
            "{encoded}"
        );
    }
}
