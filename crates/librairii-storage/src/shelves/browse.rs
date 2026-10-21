use super::*;
use librairii_core::{
    browsing::StorySummary,
    collection_codec::Query,
    query::{QueryRequest, normalize_request},
};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMember {
    pub member: Member,
    pub story: Option<StorySummary>,
}
#[derive(Debug)]
pub struct MemberPage {
    pub shelf: Shelf,
    pub members: Vec<ResolvedMember>,
    pub total: usize,
    pub page: usize,
    pub pages: usize,
}
impl Database<'_> {
    /// UUID resolution is read-only. Missing rows retain approved membership and
    /// cached titles; a later import reconnects without rewriting any decisions.
    pub fn shelf_page(
        &self,
        id: i64,
        request: QueryRequest,
        locale: String,
    ) -> Result<Pending<'_, MemberPage>> {
        self.read(move |tx| page(tx, id, request, &locale))
    }
}
pub fn page(
    tx: &Transaction<'_>,
    id: i64,
    request: QueryRequest,
    locale: &str,
) -> Result<MemberPage> {
    let sorted = !request.sort.is_empty();
    let navigation = normalize_request(request.clone())?;
    let filtered = navigation.criteria != Query::default();
    let shelf = read(tx, id)?;
    let members = members(tx, id)?;
    let uuids = members.iter().map(|m| m.uuid.clone()).collect::<Vec<_>>();
    let mut available = BTreeMap::new();
    let mut ranks = BTreeMap::new();
    crate::query::visit_uuids(tx, request, locale, &uuids, |s| {
        ranks.insert(s.uuid.clone(), ranks.len());
        available.insert(s.uuid.clone(), s);
        Ok(())
    })?;
    let mut resolved = Vec::new();
    for (position, mut member) in members.into_iter().enumerate() {
        member.position = position as i64;
        let story = available.remove(&member.uuid);
        if !filtered || story.is_some() {
            resolved.push(ResolvedMember { member, story });
        }
    }
    if sorted {
        resolved.sort_by_key(|m| ranks.get(&m.member.uuid).copied().unwrap_or(usize::MAX));
    }
    let total = resolved.len();
    let size = (navigation.page_size as usize).clamp(1, 25);
    let pages = total.div_ceil(size).max(1);
    let page = (navigation.page as usize).clamp(1, pages);
    let members = resolved
        .into_iter()
        .skip((page - 1) * size)
        .take(size)
        .collect();
    Ok(MemberPage {
        shelf,
        members,
        total,
        page,
        pages,
    })
}
