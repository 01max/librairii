//! An explicit retry owns the exact input returned before scheduler admission.
//! Observation errors and missing SQL/HTTP results cannot create retry authority.
use librairii_app::{lifecycle::LibraryGeneration, native_jobs::Start};
use librairii_storage::{Result, StorageError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefusedSync {
    pub id: String,
    pub revision: u64,
    pub generation: LibraryGeneration,
}

#[derive(Debug, Default)]
pub(super) struct Submission {
    refused: Option<(RefusedSync, Start)>,
}

impl Submission {
    pub fn receipt(&self) -> Option<RefusedSync> {
        self.refused.as_ref().map(|(receipt, _)| receipt.clone())
    }

    pub fn retry(
        &mut self,
        receipt: &RefusedSync,
        generation: &LibraryGeneration,
    ) -> Result<Start> {
        if &receipt.generation != generation
            || self
                .refused
                .as_ref()
                .is_none_or(|(saved, _)| saved != receipt)
        {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(self.refused.take().expect("checked refusal").1)
    }

    pub fn offer(
        &mut self,
        start: Start,
        generation: LibraryGeneration,
        revision: u64,
        offer: impl FnOnce(Start) -> std::result::Result<(), (StorageError, Start)>,
    ) -> Result<()> {
        self.refused = None;
        match offer(start) {
            Ok(()) => Ok(()),
            Err((StorageError::QueueFull, start)) => {
                self.refused = Some((
                    RefusedSync {
                        id: start.input().id.clone(),
                        revision,
                        generation,
                    },
                    start,
                ));
                Err(StorageError::QueueFull)
            }
            Err((error, _)) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use librairii_app::{
        lifecycle::LifecycleGate,
        ports::creation::{NewItem, NewOperation},
    };
    fn input() -> Start {
        Start::new(NewOperation {
            id: "a0000000-0000-4000-8000-000000000097".into(),
            kind: librairii_core::OperationKind::MetadataSync,
            source: None,
            destination: None,
            items: vec![NewItem {
                source_name: "en-GB".into(),
                ..Default::default()
            }],
        })
        .unwrap()
    }
    #[test]
    fn retry_keeps_owned_input_and_requires_current_receipt_and_generation() {
        let gate = LifecycleGate::new();
        let generation = gate.enter().unwrap().generation().clone();
        let mut submission = Submission::default();
        let start = input();
        let address = start.input() as *const NewOperation;
        assert!(
            submission
                .offer(start, generation.clone(), 4, |s| Err((
                    StorageError::QueueFull,
                    s
                )))
                .is_err()
        );
        let first = submission.receipt().unwrap();
        let other = LifecycleGate::new();
        assert!(
            submission
                .retry(&first, other.enter().unwrap().generation())
                .is_err()
        );
        let mut wrong = first.clone();
        wrong.revision += 1;
        assert!(submission.retry(&wrong, &generation).is_err());
        let mut wrong = first.clone();
        wrong.id.push('x');
        assert!(submission.retry(&wrong, &generation).is_err());
        let start = submission.retry(&first, &generation).unwrap();
        assert_eq!(start.input() as *const NewOperation, address);
        assert!(submission.retry(&first, &generation).is_err());
        assert!(
            submission
                .offer(start, generation.clone(), 5, |s| Err((
                    StorageError::QueueFull,
                    s
                )))
                .is_err()
        );
        assert!(submission.retry(&first, &generation).is_err());
        let second = submission.receipt().unwrap();
        let start = submission.retry(&second, &generation).unwrap();
        let mut submitted = 0;
        submission
            .offer(start, generation.clone(), 6, |s| {
                assert_eq!(s.input() as *const NewOperation, address);
                submitted += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(submitted, 1);
        assert!(submission.receipt().is_none());
        // Even a later observation failure cannot recover the consumed input.
        assert!(submission.retry(&second, &generation).is_err());
    }
    #[test]
    fn closed_or_fenced_offers_never_authorize_retry() {
        let gate = LifecycleGate::new();
        let generation = gate.enter().unwrap().generation().clone();
        for error in [
            StorageError::IntakeClosed,
            StorageError::RuntimeFenced,
            StorageError::RuntimeInUse,
        ] {
            let mut submission = Submission::default();
            assert!(
                submission
                    .offer(input(), generation.clone(), 1, |s| Err((error, s)))
                    .is_err()
            );
            assert!(submission.receipt().is_none());
        }
    }
}
