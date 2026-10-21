use super::*;
use librairii_core::{
    collections::{Collection, CollectionError},
    query::{QueryInputError, QueryRequest},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceState {
    pub source: Source,
    pub definition: Option<Collection>,
    pub matches: Vec<String>,
    pub warning: Option<&'static str>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvaluation {
    pub sources: Vec<SourceState>,
    pub matching: BTreeMap<String, Vec<Source>>,
    pub incomplete: bool,
}
impl Database<'_> {
    pub fn attach_shelf_sources(
        &self,
        id: i64,
        ids: Vec<i64>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        self.transaction(cancellation, move |tx| {
            read(tx, id)?;
            let changed = attach(tx, id, &ids)?;
            bump(tx, id, changed)
        })
    }
    pub fn shelf_source_evaluation(
        &self,
        id: i64,
        locale: String,
    ) -> Result<Pending<'_, SourceEvaluation>> {
        self.read(move |tx| {
            read(tx, id)?;
            evaluate(tx, sources(tx, id, true)?, &locale)
        })
    }
}
pub(super) fn unique_ids(ids: &[i64]) -> Result<Vec<i64>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for id in ids {
        if *id <= 0 {
            return Err(ShelfError::InvalidInput.into());
        }
        if seen.insert(*id) {
            out.push(*id);
        }
    }
    Ok(out)
}
pub(super) fn potential(tx: &Transaction<'_>, ids: &[i64]) -> Result<Vec<Source>> {
    unique_ids(ids)?
        .into_iter()
        .map(|id| {
            let c = crate::collections::read(tx, id)?;
            let name =
                String::from_utf8(c.name).map_err(|_| StorageError::CompatibilityInvariant)?;
            Ok(Source {
                id: 0,
                collection_id: Some(id),
                name,
                active: true,
            })
        })
        .collect()
}
pub(super) fn attach(tx: &Transaction<'_>, id: i64, ids: &[i64]) -> Result<bool> {
    let desired = potential(tx, ids)?;
    let mut changed = false;
    for source in desired {
        changed|=tx.execute("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(?,?,?) ON CONFLICT DO NOTHING",params![id,source.collection_id,source.name])?!=0;
    }
    Ok(changed)
}
fn domain_attention(e: &StorageError) -> bool {
    matches!(
        e,
        StorageError::Collection(
            CollectionError::NeedsAttention
                | CollectionError::CriteriaUnavailable
                | CollectionError::InvalidQuery
        ) | StorageError::CollectionNotFound
            | StorageError::QueryInput(QueryInputError::InvalidQuery)
    )
}
pub(super) fn evaluate(
    tx: &Transaction<'_>,
    sources: Vec<Source>,
    locale: &str,
) -> Result<SourceEvaluation> {
    let mut result = SourceEvaluation {
        sources: Vec::new(),
        matching: BTreeMap::new(),
        incomplete: false,
    };
    for source in sources {
        let id = source.collection_id.ok_or(ShelfError::InvalidInput)?;
        let mut state = SourceState {
            source,
            definition: None,
            matches: Vec::new(),
            warning: None,
        };
        // Invalid definitions remain exact evidence; reads never migrate or heal.
        state.definition = match crate::collections::read(tx, id) {
            Ok(c) => Some(c),
            Err(StorageError::CollectionNotFound) => None,
            Err(e) => return Err(e),
        };
        let evaluated = (|| {
            let (_, criteria) = crate::collections::source(tx, id)?;
            crate::query::visit_scope(
                tx,
                QueryRequest {
                    criteria,
                    ..Default::default()
                },
                locale,
                |story| {
                    state.matches.push(story.uuid);
                    Ok(())
                },
            )?;
            Ok(())
        })();
        if let Err(e) = evaluated {
            if !domain_attention(&e) {
                return Err(e);
            }
            state.matches.clear();
            state.warning = Some("source_needs_attention");
            result.incomplete = true;
        }
        for uuid in &state.matches {
            result
                .matching
                .entry(uuid.clone())
                .or_default()
                .push(state.source.clone());
        }
        result.sources.push(state);
    }
    Ok(result)
}
