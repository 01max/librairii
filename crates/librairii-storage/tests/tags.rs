#![cfg(unix)]
use librairii_core::tagging::{CreateDefinition, Kind, TagError};
use librairii_storage::{
    IsolatedLibrary, Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    tagging::{Command, Response},
};
use librairii_testkit::TestRoots;
use std::error::Error;
type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
fn committed<T>(v: TransactionOutcome<T>) -> Result<T> {
    match v {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e)
        }
    }
}
fn edit(db: &Database<'_>, command: Command) -> Result<Response> {
    committed(db.edit_tags(command, WorkCancellation::default())?.wait()?)
}
fn create(db: &Database<'_>, key: &str, kind: Kind) -> Result<i64> {
    match edit(
        db,
        Command::CreateDefinition(CreateDefinition {
            key: key.into(),
            label: key.into(),
            color: "#405CF5".into(),
            kind,
        }),
    )? {
        Response::Definition(d) => Ok(d.id),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
fn value(db: &Database<'_>, id: i64, key: &str) -> Result<i64> {
    match edit(
        db,
        Command::CreateValue {
            definition_id: id,
            key: key.into(),
            label: key.into(),
        },
    )? {
        Response::Value(v) => Ok(v.id),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
#[test]
fn crud_preserves_stable_keys_normalized_uniqueness_and_atomic_order() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let a = create(&db, "Été favorites", Kind::Boolean)?;
    let b = create(&db, "Genre", Kind::Choice)?;
    assert!(matches!(
        create(&db, "ete--favorites", Kind::Boolean),
        Err(StorageError::Tag(TagError::DuplicateDefinition))
    ));
    let v = value(&db, b, "Été")?;
    assert!(matches!(
        value(&db, b, "ete"),
        Err(StorageError::Tag(TagError::DuplicateValue))
    ));
    assert!(matches!(
        value(&db, a, "bad"),
        Err(StorageError::Tag(TagError::ValuesNotAllowed))
    ));
    edit(
        &db,
        Command::RenameDefinition {
            id: a,
            label: "Renamed".into(),
        },
    )?;
    edit(
        &db,
        Command::RenameValue {
            id: v,
            label: "Summer".into(),
        },
    )?;
    edit(&db, Command::ReorderDefinitions(vec![b, a]))?;
    let before = db.tag_catalog()?.wait()?;
    assert_eq!(before[1].definition.key, "ete-favorites");
    assert_eq!(before[0].values[0].key, "ete");
    assert!(edit(&db, Command::ReorderDefinitions(vec![a, a])).is_err());
    assert_eq!(db.tag_catalog()?.wait()?, before);
    assert!(
        edit(
            &db,
            Command::RecolorDefinition {
                id: a,
                color: "#FFFFFF".into()
            }
        )
        .is_err()
    );
    create(&db, "broken", Kind::Boolean)?;
    db.close()?;
    Ok(())
}
