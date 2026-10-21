use librairii_core::{
    LibraryId,
    collection_codec::url::{self, Location, Scope},
    navigation::{Frame, MAX_HISTORY, Navigation, Presentation},
    query::QueryRequest,
};
use std::error::Error;
type TestResult = Result<(), Box<dyn Error>>;
fn library() -> Result<LibraryId, librairii_core::BoundaryError> {
    "10000000-0000-4000-8000-000000000001".parse()
}
fn location(name: &str) -> Location {
    let mut request = QueryRequest::default();
    request.criteria.name = name.into();
    Location {
        scope: Scope::Custom,
        request,
    }
}

#[test]
fn navigation_restores_scope_refinement_selection_page_and_independent_presentation() -> TestResult
{
    let mut nav = Navigation::new(library()?, 1, location(" Raw Text "))?;
    nav.select(vec![7, 3])?;
    nav.set_presentation(Presentation::LargeCovers);
    let mut shelf=url::decode_history("#/library?v=3&page=4&sort=imported_desc",br##"{"collectionQuery":{"hash":"#/library?v=3&page=4&sort=imported_desc","group":{"kind":"shelf","id":7,"manualOrder":false}}}"##,0)?;
    shelf.request.criteria.name = "Moon".into();
    nav.push(Frame {
        location: shelf.clone(),
        selection: vec![9],
    })?;
    assert!(nav.back()?);
    assert_eq!(nav.current().selection, vec![7, 3]);
    assert_eq!(nav.current().location.request.criteria.name, " Raw Text ");
    assert!(nav.forward()?);
    assert_eq!(nav.current().location, shelf);
    assert_eq!(nav.presentation(), Presentation::LargeCovers);
    let restored = Navigation::decode(&nav.encode()?, library()?, 2)?;
    assert_eq!(restored.current(), nav.current());
    assert_eq!(restored.presentation(), Presentation::LargeCovers);
    assert!(!restored.accepts(nav.stamp()));
    Ok(())
}

#[test]
fn navigation_branching_bounds_and_invalid_selection_preserve_the_prior_state() -> TestResult {
    let mut nav = Navigation::new(library()?, 1, location("one"))?;
    for i in 0..MAX_HISTORY + 2 {
        nav.push(Frame {
            location: location(&i.to_string()),
            selection: Vec::new(),
        })?;
    }
    let mut count = 1;
    while nav.back()? {
        count += 1;
    }
    assert_eq!(count, MAX_HISTORY);
    assert!(nav.forward()?);
    nav.push(Frame {
        location: location("new branch"),
        selection: vec![],
    })?;
    assert!(!nav.forward()?);
    let before = nav.clone();
    assert!(nav.select(vec![1, 1]).is_err());
    assert_eq!(nav, before);
    assert!(nav.select(vec![-1]).is_err());
    assert_eq!(nav, before);
    Ok(())
}

#[test]
fn stale_query_and_library_generation_stamps_cannot_apply_results() -> TestResult {
    let mut nav = Navigation::new(library()?, 9, location("one"))?;
    let first = nav.stamp();
    nav.set_presentation(Presentation::SmallCovers);
    nav.select(vec![1])?;
    assert!(nav.accepts(first));
    nav.push(Frame {
        location: location("two"),
        selection: Vec::new(),
    })?;
    assert!(!nav.accepts(first));
    let second = nav.stamp();
    assert!(nav.back()?);
    assert!(!nav.accepts(second));
    let other = Navigation::new(
        "20000000-0000-4000-8000-000000000001".parse()?,
        9,
        location("one"),
    )?;
    assert!(!other.accepts(nav.stamp()));
    Ok(())
}

#[test]
fn reopen_preserves_one_pass_unicode_and_refuses_foreign_future_or_corrupt_documents() -> TestResult
{
    let request = url::decode("#/library?v=3&name=%F0%9D%91%A8", 0)?;
    assert_eq!(request.criteria.name, "A");
    let nav = Navigation::new(
        library()?,
        1,
        Location {
            scope: Scope::Library,
            request,
        },
    )?;
    let bytes = nav.encode()?;
    let restored = Navigation::decode(&bytes, library()?, 2)?;
    assert_eq!(restored.current().location.request.criteria.name, "A");
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes)?;
    wire["version"] = 2.into();
    assert!(Navigation::decode(&serde_json::to_vec(&wire)?, library()?, 2).is_err());
    assert!(
        Navigation::decode(&bytes, "20000000-0000-4000-8000-000000000001".parse()?, 2).is_err()
    );
    assert!(Navigation::decode(b"{", library()?, 2).is_err());
    Ok(())
}
