//! Bounded ordered intake. Only adjacent unstarted search requests coalesce.
use super::Command;
use std::{
    collections::VecDeque,
    fmt,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferError {
    Busy,
    Closed,
}
impl fmt::Display for OfferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "settings.busy",
            Self::Closed => "settings.restart_required",
        })
    }
}
impl std::error::Error for OfferError {}
#[derive(Debug)]
pub(crate) struct Envelope {
    pub revision: u64,
    pub command: Command,
}
#[derive(Debug, Default)]
struct Pending {
    queue: VecDeque<Envelope>,
    revision: u64,
}
#[derive(Debug, Default)]
pub(crate) struct Intake {
    pending: Mutex<Pending>,
    available: Condvar,
    latest: AtomicU64,
    fenced: AtomicBool,
}
impl Intake {
    pub fn offer(&self, command: Command) -> Result<u64, OfferError> {
        if matches!(&command,Command::Operations{action,..} if !action.bounded()) {
            return Err(OfferError::Busy);
        }
        let mut pending = self.pending.lock().map_err(|_| OfferError::Closed)?;
        if self.fenced.load(Ordering::Acquire)
            && !matches!(command, Command::Language(_) | Command::Refresh)
        {
            return Err(OfferError::Closed);
        }
        let coalesce = matches!(command, Command::Request(_))
            && pending
                .queue
                .back()
                .is_some_and(|e| matches!(e.command, Command::Request(_)));
        if !coalesce && pending.queue.len() == super::COMMAND_CAPACITY {
            return Err(OfferError::Busy);
        }
        let revision = pending.revision.checked_add(1).ok_or(OfferError::Closed)?;
        if coalesce {
            pending.queue.pop_back();
        }
        pending.queue.push_back(Envelope { revision, command });
        pending.revision = revision;
        self.latest.store(revision, Ordering::Release);
        self.available.notify_one();
        Ok(revision)
    }
    pub fn fence(&self) -> Result<(), OfferError> {
        let mut pending = self.pending.lock().map_err(|_| OfferError::Closed)?;
        self.fenced.store(true, Ordering::Release);
        pending
            .queue
            .retain(|entry| matches!(entry.command, Command::Language(_) | Command::Refresh));
        Ok(())
    }
    pub fn latest(&self) -> u64 {
        self.latest.load(Ordering::Acquire)
    }
    pub fn receive(&self) -> Result<Option<Envelope>, OfferError> {
        let pending = self.pending.lock().map_err(|_| OfferError::Closed)?;
        let (mut pending, _) = self
            .available
            .wait_timeout_while(pending, Duration::from_millis(20), |p| p.queue.is_empty())
            .map_err(|_| OfferError::Closed)?;
        Ok(pending.queue.pop_front())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_queries_coalesce_without_reordering_history_or_losing_shutdown_capacity() {
        let intake = Intake::default();
        let mut request = librairii_core::query::QueryRequest::default();
        request.criteria.name = "first".into();
        assert_eq!(intake.offer(Command::Request(request.clone())), Ok(1));
        request.criteria.name = "last".into();
        assert_eq!(intake.offer(Command::Request(request.clone())), Ok(2));
        assert_eq!(intake.offer(Command::Back), Ok(3));
        let first = intake.receive().ok().flatten();
        assert!(
            matches!(first,Some(Envelope{revision:2,command:Command::Request(r)})if r==request)
        );
        assert!(matches!(
            intake.receive().ok().flatten(),
            Some(Envelope {
                revision: 3,
                command: Command::Back
            })
        ));
        for _ in 0..super::super::COMMAND_CAPACITY {
            assert!(intake.offer(Command::Back).is_ok());
        }
        let admitted = intake.latest();
        assert_eq!(intake.offer(Command::Forward), Err(OfferError::Busy));
        assert_eq!(intake.latest(), admitted);
    }
}
