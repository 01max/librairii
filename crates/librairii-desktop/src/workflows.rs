//! Owned workflow observations and frozen command targets. No UI value grants
//! authority without the actual generation/query/UUID checks on the controller.
pub mod tags;
use crate::library::Runtime;
use librairii_app::lifecycle::LibraryGeneration;
use librairii_core::navigation::{Navigation, QueryStamp};
use librairii_storage::{Result, StorageError, database::WorkCancellation, query::StoryDetail};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoryTarget {
    pub generation: LibraryGeneration,
    pub query: QueryStamp,
    pub id: i64,
    pub uuid: String,
}
impl StoryTarget {
    pub fn accepts(&self, runtime: &Runtime<'_, '_, '_>, navigation: &Navigation) -> bool {
        runtime.accepts(&self.generation) && navigation.accepts(&self.query)
    }
}
#[derive(Clone, Debug)]
pub struct Details {
    pub target: StoryTarget,
    pub story: StoryDetail,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub tags: tags::Snapshot,
    pub details: Option<Details>,
    pub removal_recovery: bool,
    pub recovery_required: bool,
    pub detail_error: bool,
    pub group_error: bool,
    pub removal: Option<std::result::Result<String, String>>,
}
pub fn inspect(
    runtime: &Runtime<'_, '_, '_>,
    navigation: &Navigation,
    target: StoryTarget,
) -> Result<Details> {
    if !target.accepts(runtime, navigation) {
        return Err(StorageError::SnapshotChanged);
    }
    let mut story = runtime
        .command(|db, _, _| db.story_detail(target.id, "en-GB".into())?.wait())?
        .ok_or(StorageError::SnapshotChanged)?;
    if story.summary.uuid != target.uuid {
        return Err(StorageError::SnapshotChanged);
    }
    story.summary.description = librairii_ui::description::readable(&story.summary.description);
    Ok(Details { target, story })
}
pub fn remove(
    runtime: &Runtime<'_, '_, '_>,
    navigation: &Navigation,
    target: &StoryTarget,
    cancel: &WorkCancellation,
    mut recovery: impl FnMut(),
) -> Result<String> {
    if !target.accepts(runtime, navigation) {
        return Err(StorageError::SnapshotChanged);
    }
    runtime.command(|db, files, _| {
        let detail = db
            .story_detail(target.id, "en-GB".into())?
            .wait()?
            .ok_or(StorageError::SnapshotChanged)?;
        if detail.summary.uuid != target.uuid {
            return Err(StorageError::SnapshotChanged);
        }
        let mut removals = librairii_storage::removal::Removals::new(db, files)?;
        let removed = removals.remove(target.id, cancel);
        // Keep real owners alive if rollback needs repair. Ordinary commands are
        // fenced by the controller on this error; never discard pending custody.
        if removals.has_unsettled_removal() {
            recovery();
        }
        while removals.has_unsettled_removal() {
            if let Ok(Some(committed)) = removals.retry_rollback() {
                return Ok(committed.uuid);
            }
            if removals.has_unsettled_removal() {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        removed.map(|removed| removed.uuid)
    })
}
