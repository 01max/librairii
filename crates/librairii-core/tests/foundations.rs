use librairii_core::{
    BoundaryError, BrowseScope, Checksum, CollectionId, ReviewDecision, ShelfId, StoryUuid,
};
use proptest::prelude::*;
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn checksum_text_roundtrips_all_bytes(bytes in any::<[u8;32]>()) {
        let value = Checksum::from_bytes(bytes);
        prop_assert_eq!(value.to_string().parse::<Checksum>(), Ok(value));
    }
    #[test]
    fn uuid_text_roundtrips_all_identity_bytes(bytes in any::<[u8;16]>()) {
        let value = StoryUuid::from_bytes(bytes);
        prop_assert_eq!(value.to_string().parse::<StoryUuid>(), Ok(value));
    }
    #[test]
    fn checksum_acceptance_is_exactly_64_ascii_hex_bytes(text in ".{0,128}") {
        let expected = text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit());
        prop_assert_eq!(text.parse::<Checksum>().is_ok(),expected);
    }
    #[test]
    fn group_ids_never_cross_scope_even_at_equal_numeric_identity(id in 1..=i64::MAX) {
        let collection = CollectionId::new(id)?;
        let shelf = ShelfId::new(id)?;
        prop_assert_ne!(BrowseScope::Collection(collection), BrowseScope::Shelf(shelf));
        prop_assert_eq!(collection.get(),shelf.get());
    }
}
#[test]
fn stable_boundary_report_keeps_identity_and_refuses_default_approval() -> Result<(), BoundaryError>
{
    let collection = CollectionId::new(7)?;
    let shelf = ShelfId::new(7)?;
    assert_ne!(
        BrowseScope::Collection(collection),
        BrowseScope::Shelf(shelf)
    );
    assert_eq!(ReviewDecision::default(), ReviewDecision::NotNow);
    let report = serde_json::json!({
        "collection_id": collection.get(), "shelf_id": shelf.get(),
        "review_decision":format!("{:?}",ReviewDecision::default()),
        "checksum":Checksum::from_bytes([0xab;32]).to_string(),
        "invalid_checksum_message": BoundaryError::InvalidChecksum.to_string()
    });
    insta::assert_json_snapshot!(report, @r###"
    {
      "checksum": "abababababababababababababababababababababababababababababababab",
      "collection_id": 7,
      "invalid_checksum_message": "invalid SHA-256 identity",
      "review_decision": "NotNow",
      "shelf_id": 7
    }
    "###);
    Ok(())
}
