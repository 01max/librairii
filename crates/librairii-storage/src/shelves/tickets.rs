//! Bounded, process-local review capabilities. Consumption happens in the
//! serialized writer before revalidation; rollback/uncertainty requires refresh.
use super::*;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const LIMIT: usize = 64;
const LIFETIME: Duration = Duration::from_secs(15 * 60);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Create,
    Update,
    Sources,
    DeleteCollection,
}
#[derive(Debug)]
pub(crate) struct Ticket {
    kind: Kind,
    target: i64,
    shelves: Vec<ShelfId>,
    issued: Instant,
    live: AtomicBool,
}
impl Ticket {
    pub(super) fn claim(&self, kind: Kind, target: i64) -> Result<()> {
        self.claim_at(kind, target, Instant::now())
    }
    fn claim_at(&self, kind: Kind, target: i64, now: Instant) -> Result<()> {
        if self.kind != kind
            || self.target != target
            || now.saturating_duration_since(self.issued) >= LIFETIME
            || !self.live.swap(false, Ordering::AcqRel)
        {
            return Err(ShelfError::StaleReview.into());
        }
        Ok(())
    }
}
#[derive(Debug, Default)]
pub(crate) struct Registry {
    tickets: VecDeque<Arc<Ticket>>,
}
impl Registry {
    fn issue_at(
        &mut self,
        kind: Kind,
        target: i64,
        shelves: Vec<ShelfId>,
        now: Instant,
    ) -> Arc<Ticket> {
        self.tickets.retain(|t| {
            let valid = t.live.load(Ordering::Acquire)
                && now.saturating_duration_since(t.issued) < LIFETIME;
            if !valid {
                t.live.store(false, Ordering::Release);
            }
            valid
        });
        while self.tickets.len() >= LIMIT {
            if let Some(t) = self.tickets.pop_front() {
                t.live.store(false, Ordering::Release);
            }
        }
        let ticket = Arc::new(Ticket {
            kind,
            target,
            shelves,
            issued: now,
            live: AtomicBool::new(true),
        });
        self.tickets.push_back(ticket.clone());
        ticket
    }
}
pub(super) fn issue(
    registry: &Mutex<Registry>,
    kind: Kind,
    target: i64,
    shelves: Vec<ShelfId>,
) -> Result<Arc<Ticket>> {
    Ok(registry
        .lock()
        .map_err(|_| ShelfError::StaleReview)?
        .issue_at(kind, target, shelves, Instant::now()))
}
pub(super) fn invalidate_shelf(registry: &Mutex<Registry>, id: ShelfId) -> Result<()> {
    registry
        .lock()
        .map_err(|_| ShelfError::StaleReview)?
        .tickets
        .retain(|ticket| {
            if ticket.shelves.contains(&id) {
                ticket.live.store(false, Ordering::Release);
                false
            } else {
                true
            }
        });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifetime_kind_target_eviction_and_replay_are_fail_closed() {
        let mut registry = Registry::default();
        let now = Instant::now();
        let first = registry.issue_at(Kind::Create, 0, vec![], now);
        assert!(first.claim_at(Kind::Update, 0, now).is_err());
        assert!(first.claim_at(Kind::Create, 1, now).is_err());
        for _ in 0..LIMIT {
            registry.issue_at(Kind::Update, 2, vec![], now);
        }
        assert!(first.claim_at(Kind::Create, 0, now).is_err());
        assert_eq!(registry.tickets.len(), LIMIT);
        let last = registry.issue_at(Kind::Create, 0, vec![], now);
        assert!(last.claim_at(Kind::Create, 0, now + LIFETIME).is_err());
        let fresh = registry.issue_at(Kind::Update, 7, vec![], now + LIFETIME);
        assert_eq!(registry.tickets.len(), 1);
        assert!(fresh.claim_at(Kind::Update, 7, now + LIFETIME).is_ok());
        assert!(fresh.claim_at(Kind::Update, 7, now + LIFETIME).is_err());
    }
}
