//! Generated persistence boundaries exercised alongside explicit custody scenarios.
use librairii_core::{
    collection_codec::{self as codec, Query, url},
    query::QueryRequest,
};
use proptest::prelude::*;
fn named(name: String) -> Query {
    let mut query = Query::default();
    query.name = name;
    query
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn ascii_saved_query_encoding_is_stable(name in "[ -~]{0,128}") {
        let encoded=codec::encode(named(name))?;
        let query=codec::decode(5,encoded.as_bytes(),1)?;
        prop_assert_eq!(codec::encode(query)?,encoded);
    }
    #[test]
    fn paged_navigation_preserves_unicode_and_exact_bounds(name in ".{0,128}", page in 1_i64..5001, size in 1_i64..101, newest in any::<bool>()) {
        let request=QueryRequest{criteria:named(name),page,page_size:size,sort:if newest{"imported_desc"}else{"name_asc"}.into()};
        let bytes=url::request_bytes(&request)?;
        prop_assert_eq!(url::restore_request(&bytes)?,request);
    }
    #[test]
    fn explicit_uuid_decisions_deduplicate_in_first_selected_order(ids in proptest::collection::vec(0_u8..32,0..200)) {
        let uuids=ids.iter().map(|id|format!("16000000-0000-4000-8000-{id:012}")).collect::<Vec<_>>();
        let mut expected=Vec::new();
        for value in &uuids {if !expected.contains(value){expected.push(value.clone());}}
        prop_assert_eq!(librairii_core::shelves::unique_uuids(&uuids)?,expected);
    }
}

#[test]
fn retained_unicode_seed_preserves_observable_frozen_canonical_passes()
-> Result<(), Box<dyn std::error::Error>> {
    // Lowercase precedes compatibility decomposition in the frozen Go recipe.
    // First pass exposes capital A; a later saved decode lowers that result.
    let first = codec::encode(named("𝕬".into()))?;
    assert_eq!(first, r#"{"name":"A"}"#);
    let decoded = codec::decode(5, first.as_bytes(), 1)?;
    assert_eq!(decoded.name, "a");
    let raw = codec::payload(&named("𝕬".into()))?;
    assert_eq!(codec::migrate(5, raw.as_bytes(), 1)?.0, r#"{"name":"a"}"#);
    Ok(())
}
