use librairii_core::collection_codec::{decode, decode_references};
#[test]
fn tag_reference_inspection_is_bounded_strict_and_distinct_from_membership_normalization()
-> Result<(), librairii_core::collection_codec::CodecError> {
    let payload=serde_json::json!({"name":"a".repeat(201),"languages":["not a locale"],"compatibilities":["unsupported"],"anyOf":[{"booleanFilters":[{"definitionId":9,"state":"true"},{"definitionId":10,"state":"ignored"}]},{"choiceFilters":[{"definitionId":11,"valueIds":vec![20;101]}]}]}).to_string();
    assert!(decode(5, payload.as_bytes(), 9).is_err());
    let refs = decode_references(5, payload.as_bytes(), 9)?;
    assert_eq!(refs.boolean_filters.len(), 1);
    assert_eq!(refs.choice_filters[0].value_ids.len(), 101);
    assert!(
        decode_references(3, payload.as_bytes(), 9)?
            .boolean_filters
            .is_empty()
    );
    assert_eq!(
        decode_references(4, payload.as_bytes(), 9)?
            .boolean_filters
            .len(),
        1
    );
    for invalid in [
        r#"{"unknown":1}"#,
        r#"{"name":"a","Name":"b"}"#,
        r#"{"booleanFilters":[{"definitionId":0,"state":"true"}]}"#,
        r#"{"choiceFilters":[{"definitionId":1,"valueIds":[]}]}"#,
        r#"{"booleanFilters":[{"definitionId":1.0,"state":"true"}]}"#,
        r#"{"includeBroken":true}"#,
    ] {
        assert!(
            decode_references(5, invalid.as_bytes(), 0).is_err(),
            "{invalid}"
        );
    }
    let oversized = format!(r#"{{"name":"{}"}}"#, "a".repeat(262_144));
    assert!(decode_references(5, oversized.as_bytes(), 0).is_err());
    let nodes = serde_json::json!({"anyOf":vec![serde_json::json!({});50]}).to_string();
    assert!(decode_references(5, nodes.as_bytes(), 0).is_err());
    Ok(())
}
