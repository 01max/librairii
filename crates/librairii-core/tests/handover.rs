use librairii_core::{LibraryId, collection_codec::url::Scope, handover, navigation::Presentation};
use std::error::Error;

const FIXTURE: &[u8] = include_bytes!("fixtures/handover-v1.json");

#[test]
fn actual_go_handover_preserves_history_and_separates_overlapping_group_ids()
-> Result<(), Box<dyn Error>> {
    let library: LibraryId = "f1000000-0000-4000-8000-000000000017".parse()?;
    let preferences = handover::decode(FIXTURE, library, 1)?;
    let mut navigation = preferences.navigation;
    assert_eq!(navigation.presentation(), Presentation::SmallCovers);
    assert!(matches!(navigation.current().location.scope, Scope::Collection(id) if id.get() == 7));
    assert_eq!(navigation.current().location.request.page, 3);
    assert_eq!(
        navigation.current().location.request.criteria.name,
        "bonjour e"
    );
    assert_eq!(
        navigation
            .current()
            .location
            .request
            .criteria
            .boolean_filters
            .len(),
        1
    );
    assert_eq!(
        navigation
            .current()
            .location
            .request
            .criteria
            .boolean_filters[0]
            .definition_id,
        7
    );
    assert!(navigation.back()?);
    assert_eq!(navigation.current().location.scope, Scope::Library);
    assert!(navigation.forward()?);
    assert!(navigation.forward()?);
    assert!(
        matches!(navigation.current().location.scope, Scope::Shelf{id, manual_order:false} if id.get() == 7)
    );
    // Current-version ordinary tag3 remains; only the old frame retired it.
    assert_eq!(
        navigation
            .current()
            .location
            .request
            .criteria
            .boolean_filters[0]
            .definition_id,
        3
    );
    assert!(preferences.sync_panel_hidden);
    assert_eq!(preferences.dismissed_operations.len(), 1);
    Ok(())
}

#[test]
fn handover_refuses_wrong_library_versions_unknown_fields_duplicates_and_bounds()
-> Result<(), Box<dyn Error>> {
    let library: LibraryId = "f1000000-0000-4000-8000-000000000017".parse()?;
    let other: LibraryId = "f1000000-0000-4000-8000-000000000018".parse()?;
    assert!(handover::decode(FIXTURE, other, 1).is_err());
    assert!(handover::decode(FIXTURE, library, 0).is_err());
    let text = std::str::from_utf8(FIXTURE)?;
    for invalid in [
        text.replace("\"version\":1", "\"version\":2"),
        text.replace("\"version\":1", "\"version\":1,\"version\":1"),
        text.replace("\"version\":1", "\"version\":1,\"browserStorage\":{}"),
        text.replace("\"position\":1", "\"position\":100"),
        text.replace("\"id\":7", "\"id\":0"),
        format!(
            "{}{}",
            " ".repeat(librairii_core::navigation::MAX_STATE_BYTES),
            text
        ),
    ] {
        assert!(handover::decode(invalid.as_bytes(), library, 1).is_err());
    }
    Ok(())
}
