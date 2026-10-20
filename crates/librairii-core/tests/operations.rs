//! PURE6 partial-6.1 assertions. No repository/runtime/effect qualification.
use librairii_core::operations::*;
use librairii_core::{BoundaryError, OperationKind, OperationState};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const STATES: [OperationState; 7] = [
    OperationState::Queued,
    OperationState::Running,
    OperationState::Succeeded,
    OperationState::PartiallySucceeded,
    OperationState::Failed,
    OperationState::Cancelled,
    OperationState::Interrupted,
];
const ITEMS: [ItemState; 7] = [
    ItemState::Pending,
    ItemState::Running,
    ItemState::Succeeded,
    ItemState::Skipped,
    ItemState::Conflicted,
    ItemState::Failed,
    ItemState::Cancelled,
];

fn outcome(code: &str, message: &str) -> RawOutcome {
    RawOutcome {
        code: Some(code.into()),
        message: Some(message.into()),
    }
}

fn source(kind: ExportSourceType, ids: Vec<i64>, names: Vec<String>) -> ExportSource {
    let mut value = ExportSource {
        source_type: kind,
        collection_ids: vec![],
        collection_names: vec![],
        shelf_ids: vec![],
        shelf_names: vec![],
    };
    if matches!(
        kind,
        ExportSourceType::Collection | ExportSourceType::Collections
    ) {
        value.collection_ids = ids;
        value.collection_names = names;
    } else {
        value.shelf_ids = ids;
        value.shelf_names = names;
    }
    value
}

fn item(id: i64, state: ItemState) -> ItemRecord {
    ItemRecord {
        identity: StoredItemIdentity {
            id,
            story_id: None,
            resolved_story_id: Some(77),
            story_uuid: Some("AABBCCDD-1234-5678-9012-AABBCCDDEEFF".into()),
        },
        state,
        progress: Progress {
            completed: 3,
            total: 10,
        },
        outcome: outcome("future_outcome", "Retained É/e\u{301}"),
    }
}

#[test]
fn op6_kind_and_operation_state_tags_match_frozen_go() -> TestResult {
    for (tag, kind) in [
        ("import", OperationKind::Import),
        ("metadata_sync", OperationKind::MetadataSync),
        ("export", OperationKind::Export),
    ] {
        assert_eq!(decode_kind(tag)?, kind);
        assert_eq!(encode_kind(kind), tag);
    }
    for (tag, state) in [
        ("queued", OperationState::Queued),
        ("running", OperationState::Running),
        ("succeeded", OperationState::Succeeded),
        ("partially_succeeded", OperationState::PartiallySucceeded),
        ("failed", OperationState::Failed),
        ("cancelled", OperationState::Cancelled),
        ("interrupted", OperationState::Interrupted),
    ] {
        assert_eq!(decode_state(tag)?, state);
        assert_eq!(encode_state(state), tag);
        assert_eq!(is_terminal(state), !["queued", "running"].contains(&tag));
    }
    Ok(())
}

#[test]
fn op6_item_tags_are_seven_distinct_legacy_states() -> TestResult {
    for (tag, state) in [
        ("pending", ItemState::Pending),
        ("running", ItemState::Running),
        ("succeeded", ItemState::Succeeded),
        ("skipped", ItemState::Skipped),
        ("conflicted", ItemState::Conflicted),
        ("failed", ItemState::Failed),
        ("cancelled", ItemState::Cancelled),
    ] {
        assert_eq!(ItemState::from_persisted(tag)?, state);
        assert_eq!(state.as_persisted(), tag);
        assert_eq!(state.is_terminal(), !["pending", "running"].contains(&tag));
    }
    Ok(())
}

#[test]
fn op6_all_eight_sources_keep_literal_tags_and_historical_meaning() -> TestResult {
    for (tag, kind, meaning) in [
        ("selection", ExportSourceType::Selection, None),
        ("current_query", ExportSourceType::CurrentQuery, None),
        (
            "shelf",
            ExportSourceType::Shelf,
            Some(ExportGroupKind::Collection),
        ),
        (
            "shelves",
            ExportSourceType::Shelves,
            Some(ExportGroupKind::Collection),
        ),
        (
            "collection",
            ExportSourceType::Collection,
            Some(ExportGroupKind::Collection),
        ),
        (
            "collections",
            ExportSourceType::Collections,
            Some(ExportGroupKind::Collection),
        ),
        (
            "curated_shelf",
            ExportSourceType::CuratedShelf,
            Some(ExportGroupKind::CuratedShelf),
        ),
        (
            "curated_shelves",
            ExportSourceType::CuratedShelves,
            Some(ExportGroupKind::CuratedShelf),
        ),
    ] {
        assert_eq!(ExportSourceType::from_persisted(tag)?, kind);
        assert_eq!(kind.as_persisted(), tag);
        assert_eq!(kind.group_kind(), meaning);
    }
    Ok(())
}

#[test]
fn op6_closed_tags_reject_aliases_case_whitespace_and_embedded_nul() {
    for text in [
        "",
        "Import",
        " import",
        "import ",
        "import\0",
        "future_kind",
    ] {
        assert_eq!(decode_kind(text), Err(OperationCodecError::UnknownKind));
    }
    for text in ["", "succeeded ", "partiallySucceeded", "pending", "unknown"] {
        assert_eq!(decode_state(text), Err(OperationCodecError::UnknownState));
    }
    for text in ["queued", "partial", "SKIPPED", "skipped\0"] {
        assert_eq!(
            ItemState::from_persisted(text),
            Err(OperationCodecError::UnknownItemState)
        );
    }
    for text in ["", "shelf_union", "Shelf", "curated-shelf", "shelf "] {
        assert_eq!(
            ExportSourceType::from_persisted(text),
            Err(OperationCodecError::UnknownExportSourceType)
        );
    }
}

#[test]
fn op6_raw_operation_identity_keeps_spelling_and_non_uuid_history() -> TestResult {
    let mut raw = StoredOperationIdentity {
        id: "AABBCCDD-1234-5678-9012-AABBCCDDEEFF".into(),
        created_at: "2000-01-01T00:00:00Z".into(),
        started_at: Some("2000-01-01 00:00:00".into()),
        finished_at: None,
    };
    let before = raw.clone();
    assert_eq!(
        raw.typed_id()?.to_string(),
        "aabbccdd-1234-5678-9012-aabbccddeeff"
    );
    assert_eq!(raw, before);
    raw.id = "legacy:operation-2".into();
    assert_eq!(raw.typed_id(), Err(BoundaryError::InvalidUuid));
    assert_eq!(raw.id, "legacy:operation-2");
    assert_eq!(raw.started_at.as_deref(), Some("2000-01-01 00:00:00"));
    assert_eq!(raw.finished_at, None);
    Ok(())
}

#[test]
fn op6_item_identity_retains_null_empty_uuid_and_resolved_story_after_removal() -> TestResult {
    let original = item(3, ItemState::Skipped).identity;
    assert_eq!(original.historical_story_id(), 77);
    assert_eq!(
        original.typed_story_uuid()?.map(|id| id.to_string()),
        Some("aabbccdd-1234-5678-9012-aabbccddeeff".into())
    );
    assert_eq!(
        original.story_uuid.as_deref(),
        Some("AABBCCDD-1234-5678-9012-AABBCCDDEEFF")
    );
    let absent = StoredItemIdentity {
        id: 1,
        story_id: None,
        resolved_story_id: None,
        story_uuid: None,
    };
    let empty = StoredItemIdentity {
        story_uuid: Some(String::new()),
        ..absent.clone()
    };
    assert_eq!(absent.typed_story_uuid()?, None);
    assert_eq!(empty.typed_story_uuid(), Err(BoundaryError::InvalidUuid));
    assert_ne!(absent, empty);
    assert_eq!(absent.historical_story_id(), 0);
    assert_eq!(
        StoredItemIdentity {
            story_id: Some(12),
            ..absent
        }
        .historical_story_id(),
        12
    );
    Ok(())
}

#[test]
fn op6_history_key_uses_raw_time_and_id_tuple_order() {
    let space = HistoryKey {
        created_at: "2000-01-01 00:00:00".into(),
        id: "z".into(),
    };
    let iso = HistoryKey {
        created_at: "2000-01-01T00:00:00Z".into(),
        id: "a".into(),
    };
    assert!(space < iso);
    let first = HistoryKey {
        created_at: "same".into(),
        id: "legacy:10".into(),
    };
    let second = HistoryKey {
        created_at: "same".into(),
        id: "legacy:2".into(),
    };
    assert!(first < second);
    assert_eq!(first.created_at, "same");
}

#[test]
fn op6_raw_scope_and_open_outcomes_are_not_normalized() -> TestResult {
    let raw = StoredExportScope {
        discriminator: Some("shelves".into()),
        group_ids: Some("[ 9, 7 ]".into()),
        group_names: Some("[\"E\\u0301\", \"É\"]".into()),
    };
    let before = raw.clone();
    assert_eq!(raw.source_type()?, Some(ExportSourceType::Shelves));
    assert_eq!(raw, before);
    assert_eq!(StoredExportScope::default().source_type()?, None);
    assert_ne!(raw.group_ids, Some("[9,7]".into()));
    let unknown = StoredExportScope {
        discriminator: Some("future_scope".into()),
        ..raw
    };
    assert_eq!(
        unknown.source_type(),
        Err(OperationCodecError::UnknownExportSourceType)
    );
    assert_eq!(unknown.discriminator.as_deref(), Some("future_scope"));
    let open = outcome("future.code/unknown", "raw E\u{301}\0message");
    assert_eq!(open.code.as_deref(), Some("future.code/unknown"));
    assert_eq!(open.message.as_deref(), Some("raw E\u{301}\0message"));
    assert_ne!(RawOutcome::default(), outcome("", ""));
    Ok(())
}

#[test]
fn op6_source_validation_preserves_order_and_separates_equal_group_ids() -> TestResult {
    for kind in [
        ExportSourceType::Shelf,
        ExportSourceType::Collection,
        ExportSourceType::CuratedShelf,
    ] {
        let source = source(kind, vec![7], vec!["É/e\u{301}".into()]);
        source.validate()?;
        assert_eq!(source.group_ids(), [7]);
        assert_eq!(source.group_names(), ["É/e\u{301}"]);
        assert_eq!(
            source.collection_history_ids().len(),
            usize::from(kind != ExportSourceType::CuratedShelf)
        );
    }
    for kind in [
        ExportSourceType::Shelves,
        ExportSourceType::Collections,
        ExportSourceType::CuratedShelves,
    ] {
        let source = source(kind, vec![9, 7], vec!["Z".into(), "A".into()]);
        source.validate()?;
        assert_eq!(source.group_ids(), [9, 7]);
        assert_eq!(source.group_names(), ["Z", "A"]);
    }
    for kind in [ExportSourceType::Selection, ExportSourceType::CurrentQuery] {
        source(kind, vec![], vec![]).validate()?;
    }
    Ok(())
}

#[test]
fn op6_source_rejects_mixed_domains_cardinality_duplicates_and_blank_names() {
    let bad = [
        source(ExportSourceType::Selection, vec![1], vec!["x".into()]),
        source(ExportSourceType::Shelf, vec![], vec![]),
        source(
            ExportSourceType::Collection,
            vec![1, 2],
            vec!["a".into(), "b".into()],
        ),
        source(ExportSourceType::Shelves, vec![1], vec!["a".into()]),
        source(
            ExportSourceType::CuratedShelves,
            vec![1, 1],
            vec!["a".into(), "b".into()],
        ),
        source(ExportSourceType::CuratedShelf, vec![0], vec!["a".into()]),
        source(ExportSourceType::Shelf, vec![-1], vec!["a".into()]),
        source(ExportSourceType::Shelf, vec![1], vec!["\u{2003}\t".into()]),
        source(ExportSourceType::Shelf, vec![1], vec![]),
    ];
    for source in bad {
        let original = source.clone();
        assert_eq!(
            source.validate(),
            Err(OperationPolicyError::InvalidExportSource)
        );
        assert_eq!(source, original);
    }
    let mut mixed = source(ExportSourceType::Collection, vec![1], vec!["a".into()]);
    mixed.shelf_ids = vec![1];
    assert_eq!(
        mixed.validate(),
        Err(OperationPolicyError::InvalidExportSource)
    );
    let mut legacy = source(ExportSourceType::Shelf, vec![1], vec!["a".into()]);
    legacy.collection_ids = vec![1];
    assert_eq!(
        legacy.validate(),
        Err(OperationPolicyError::InvalidExportSource)
    );
}

#[test]
fn op6_provisional_interruption_is_not_a_saved_terminal_snapshot() {
    for state in STATES {
        assert_eq!(
            OperationObservation {
                state,
                kind: ObservationKind::StoredSnapshot
            }
            .is_saved_terminal(),
            is_terminal(state)
        );
        assert!(
            !OperationObservation {
                state,
                kind: ObservationKind::ProvisionalPersistenceFailure
            }
            .is_saved_terminal()
        );
    }
}

#[test]
fn op6_start_and_cancel_refuse_all_terminal_parents() {
    for state in STATES {
        if [OperationState::Queued, OperationState::Running].contains(&state) {
            assert_eq!(mark_running(state), Ok(OperationState::Running));
            assert_eq!(request_cancel(state), Ok(true));
        } else {
            assert_eq!(
                mark_running(state),
                Err(OperationPolicyError::InactiveOperation)
            );
            assert_eq!(
                request_cancel(state),
                Err(OperationPolicyError::InactiveOperation)
            );
        }
    }
    for state in ITEMS {
        assert_eq!(
            mark_item_running(state),
            if state == ItemState::Pending {
                Ok(ItemState::Running)
            } else {
                Err(OperationPolicyError::InvalidTransition)
            }
        );
    }
}

#[test]
fn op6_progress_is_running_only_monotone_and_within_total() {
    let progress = Progress {
        completed: 3,
        total: 10,
    };
    for state in ITEMS {
        assert_eq!(
            progress.advance(state, 4).is_ok(),
            state == ItemState::Running
        );
    }
    assert_eq!(progress.advance(ItemState::Running, 3), Ok(progress));
    assert_eq!(
        progress.advance(ItemState::Running, 10),
        Ok(Progress {
            completed: 10,
            total: 10
        })
    );
    for next in [-1, 2, 11, i64::MAX] {
        assert_eq!(
            progress.advance(ItemState::Running, next),
            Err(OperationPolicyError::InvalidProgress)
        );
    }
    assert_eq!(
        Progress {
            completed: 0,
            total: 0
        }
        .advance(ItemState::Running, 0),
        Ok(Progress {
            completed: 0,
            total: 0
        })
    );
    assert!(
        Progress {
            completed: -1,
            total: 3
        }
        .advance(ItemState::Running, 0)
        .is_err()
    );
    assert!(
        Progress {
            completed: 0,
            total: -1
        }
        .advance(ItemState::Running, 0)
        .is_err()
    );
}

#[test]
fn op6_completion_candidate_increments_once_and_rejects_repeated_terminal_item() -> TestResult {
    for parent in [OperationState::Queued, OperationState::Running] {
        for current in [ItemState::Pending, ItemState::Running] {
            for terminal in ITEMS.into_iter().filter(|state| state.is_terminal()) {
                let candidate = complete_item(
                    parent,
                    current,
                    Counts {
                        completed: 0,
                        total: 1,
                    },
                    terminal,
                )?;
                assert_eq!(
                    candidate.counts,
                    Counts {
                        completed: 1,
                        total: 1
                    }
                );
                assert_eq!(candidate.item_state, terminal);
                assert_eq!(
                    complete_item(parent, candidate.item_state, candidate.counts, terminal),
                    Err(OperationPolicyError::InvalidTransition)
                );
            }
        }
    }
    Ok(())
}

#[test]
fn op6_completion_rejects_inactive_parent_nonterminal_result_and_bad_counts() {
    for state in STATES.into_iter().filter(|state| is_terminal(*state)) {
        assert_eq!(
            complete_item(
                state,
                ItemState::Pending,
                Counts {
                    completed: 0,
                    total: 1
                },
                ItemState::Succeeded
            ),
            Err(OperationPolicyError::InactiveOperation)
        );
    }
    for target in [ItemState::Pending, ItemState::Running] {
        assert_eq!(
            complete_item(
                OperationState::Running,
                ItemState::Pending,
                Counts {
                    completed: 0,
                    total: 1
                },
                target
            ),
            Err(OperationPolicyError::InvalidTransition)
        );
    }
    for counts in [
        Counts {
            completed: 0,
            total: 0,
        },
        Counts {
            completed: -1,
            total: 1,
        },
        Counts {
            completed: 2,
            total: 1,
        },
        Counts {
            completed: 1,
            total: 1,
        },
    ] {
        assert!(
            complete_item(
                OperationState::Running,
                ItemState::Pending,
                counts,
                ItemState::Succeeded
            )
            .is_err()
        );
    }
}

#[test]
fn op6_finish_requires_all_items_and_preserves_existing_terminal_truth() {
    for target in STATES {
        let result = finish(
            OperationState::Running,
            Counts {
                completed: 1,
                total: 1,
            },
            target,
        );
        assert_eq!(
            result.is_ok(),
            is_terminal(target) && target != OperationState::Interrupted
        );
        if result.is_ok() {
            assert_eq!(result, Ok(target));
        }
    }
    assert_eq!(
        finish(
            OperationState::Running,
            Counts {
                completed: 0,
                total: 1
            },
            OperationState::Succeeded
        ),
        Err(OperationPolicyError::IncompleteOperation)
    );
    for existing in STATES.into_iter().filter(|state| is_terminal(*state)) {
        assert_eq!(
            finish(
                existing,
                Counts {
                    completed: 1,
                    total: 1
                },
                OperationState::Failed
            ),
            Err(OperationPolicyError::InactiveOperation)
        );
    }
}

#[test]
fn op6_interruption_preserves_terminal_payload_ids_and_partial_bytes() -> TestResult {
    let original: Vec<_> = ITEMS
        .into_iter()
        .enumerate()
        .map(|(index, state)| item(index as i64 + 1, state))
        .collect();
    let reason = outcome("interrupted", "Operation was interrupted.");
    let candidate = interrupt(
        OperationState::Running,
        Counts {
            completed: 5,
            total: 7,
        },
        &original,
        &reason,
    )?;
    assert_eq!(candidate.state, OperationState::Interrupted);
    assert_eq!(
        candidate.counts,
        Counts {
            completed: 7,
            total: 7
        }
    );
    for (before, after) in original.iter().zip(&candidate.items) {
        assert_eq!(after.identity, before.identity);
        assert_eq!(after.progress, before.progress);
        if before.state.is_terminal() {
            assert_eq!(after, before);
        } else {
            assert_eq!(after.state, ItemState::Cancelled);
            assert_eq!(after.outcome, reason);
        }
    }
    assert_eq!(original[0].state, ItemState::Pending);
    Ok(())
}

#[test]
fn op6_interruption_rejects_missing_reason_inconsistent_scope_and_terminal_parent() {
    let items = [item(1, ItemState::Running)];
    let counts = Counts {
        completed: 0,
        total: 1,
    };
    for reason in [
        RawOutcome::default(),
        outcome("", "message"),
        outcome("code", ""),
    ] {
        assert_eq!(
            interrupt(OperationState::Running, counts, &items, &reason),
            Err(OperationPolicyError::InvalidInterruption)
        );
    }
    let reason = outcome("interrupted", "reason");
    assert_eq!(
        interrupt(OperationState::Succeeded, counts, &items, &reason),
        Err(OperationPolicyError::InactiveOperation)
    );
    assert_eq!(
        interrupt(
            OperationState::Running,
            Counts {
                completed: 0,
                total: 2
            },
            &items,
            &reason
        ),
        Err(OperationPolicyError::InvalidCounts)
    );
    assert_eq!(
        interrupt(
            OperationState::Running,
            Counts {
                completed: 1,
                total: 1
            },
            &items,
            &reason
        ),
        Err(OperationPolicyError::InvalidCounts)
    );
}

#[test]
fn op6_signed_byte_totals_reject_negative_and_overflow_without_wrapping() {
    assert_eq!(checked_byte_total(&[0, i64::MAX]), Ok(i64::MAX));
    assert_eq!(checked_byte_total(&[]), Ok(0));
    assert_eq!(
        checked_byte_total(&[i64::MAX, 1]),
        Err(OperationPolicyError::ByteTotalOverflow)
    );
    assert_eq!(
        checked_byte_total(&[1, -1]),
        Err(OperationPolicyError::NegativeByteCount)
    );
}

#[test]
fn op6_aggregate_exact_kind_precedence_matches_go_manager() -> TestResult {
    use ItemState::{Cancelled as C, Conflicted as X, Failed as F, Skipped as K, Succeeded as S};
    let cases: &[(OperationKind, &[ItemState], OperationState, &str)] = &[
        (OperationKind::Import, &[S], OperationState::Succeeded, ""),
        (OperationKind::Import, &[K], OperationState::Succeeded, ""),
        (
            OperationKind::Import,
            &[X],
            OperationState::Failed,
            "import_failed",
        ),
        (
            OperationKind::Import,
            &[S, X],
            OperationState::PartiallySucceeded,
            "partial_failure",
        ),
        (
            OperationKind::Import,
            &[K, C],
            OperationState::PartiallySucceeded,
            "partial_failure",
        ),
        (
            OperationKind::Import,
            &[C],
            OperationState::Cancelled,
            "cancelled",
        ),
        (
            OperationKind::Import,
            &[F, C],
            OperationState::Failed,
            "import_failed",
        ),
        (
            OperationKind::MetadataSync,
            &[S],
            OperationState::Succeeded,
            "",
        ),
        (
            OperationKind::MetadataSync,
            &[C],
            OperationState::Cancelled,
            "catalog_cancelled",
        ),
        (
            OperationKind::MetadataSync,
            &[F],
            OperationState::Failed,
            "catalog_refresh_failed",
        ),
        (OperationKind::Export, &[S], OperationState::Succeeded, ""),
        (
            OperationKind::Export,
            &[K],
            OperationState::Failed,
            "export_failed",
        ),
        (
            OperationKind::Export,
            &[X],
            OperationState::Failed,
            "export_failed",
        ),
        (
            OperationKind::Export,
            &[S, K],
            OperationState::PartiallySucceeded,
            "partial_export",
        ),
        (
            OperationKind::Export,
            &[S, X],
            OperationState::PartiallySucceeded,
            "partial_export",
        ),
        (
            OperationKind::Export,
            &[S, F, C],
            OperationState::PartiallySucceeded,
            "partial_export",
        ),
        (
            OperationKind::Export,
            &[X, C],
            OperationState::Cancelled,
            "cancelled",
        ),
        (
            OperationKind::Export,
            &[F, C],
            OperationState::Failed,
            "export_failed",
        ),
    ];
    for (kind, states, expected, code) in cases {
        let mut aggregate = CompletionAggregate::new(*kind, states.len() as i64)?;
        for state in *states {
            aggregate.record(*state, &RawOutcome::default())?;
        }
        let result = aggregate.terminal_result()?;
        assert_eq!(
            (result.state, result.code.as_str()),
            (*expected, *code),
            "kind={kind:?} items={states:?}"
        );
        assert_eq!(aggregate.counts().completed, states.len() as i64);
    }
    Ok(())
}

#[test]
fn op6_aggregate_failure_code_sentinel_and_message_fallback_are_independent() -> TestResult {
    for kind in [OperationKind::Export, OperationKind::MetadataSync] {
        let first = outcome("", "first empty-code message");
        let second = outcome("future_error", "second message");
        let mut aggregate = CompletionAggregate::new(kind, 2)?;
        aggregate.record(ItemState::Failed, &first)?;
        aggregate.record(ItemState::Failed, &second)?;
        let result = aggregate.terminal_result()?;
        assert_eq!(
            (result.code.as_str(), result.message.as_str()),
            ("future_error", "second message")
        );
        assert_eq!(first.code.as_deref(), Some(""));
        let mut aggregate = CompletionAggregate::new(kind, 2)?;
        aggregate.record(ItemState::Failed, &outcome("first", ""))?;
        aggregate.record(ItemState::Failed, &second)?;
        let result = aggregate.terminal_result()?;
        assert_eq!(result.code, "first");
        assert_eq!(
            result.message,
            if kind == OperationKind::Export {
                "No stories were exported."
            } else {
                "Official metadata could not be refreshed."
            }
        );
    }
    Ok(())
}

#[test]
fn op6_aggregate_first_failure_follows_ack_order_not_item_sort_order() -> TestResult {
    for codes in [["z-first", "a-second"], ["a-second", "z-first"]] {
        let mut aggregate = CompletionAggregate::new(OperationKind::Export, 2)?;
        for code in codes {
            aggregate.record(ItemState::Failed, &outcome(code, code))?;
        }
        assert_eq!(aggregate.terminal_result()?.code, codes[0]);
    }
    Ok(())
}

#[test]
fn op6_aggregate_rejects_unfinished_and_extra_items_without_partial_mutation() -> TestResult {
    assert_eq!(
        CompletionAggregate::new(OperationKind::Import, 0),
        Err(OperationPolicyError::InvalidCounts)
    );
    let mut aggregate = CompletionAggregate::new(OperationKind::Import, 1)?;
    assert_eq!(
        aggregate.terminal_result(),
        Err(OperationPolicyError::IncompleteOperation)
    );
    for state in [ItemState::Pending, ItemState::Running] {
        let before = aggregate.clone();
        assert_eq!(
            aggregate.record(state, &RawOutcome::default()),
            Err(OperationPolicyError::InvalidTransition)
        );
        assert_eq!(aggregate, before);
    }
    aggregate.record(ItemState::Succeeded, &outcome("imported", ""))?;
    let before = aggregate.clone();
    assert_eq!(
        aggregate.record(ItemState::Failed, &outcome("late", "late")),
        Err(OperationPolicyError::InvalidCounts)
    );
    assert_eq!(aggregate, before);
    assert_eq!(
        aggregate.terminal_result()?.state,
        OperationState::Succeeded
    );
    Ok(())
}

#[test]
fn op6_admitted_history_keeps_typed_states_raw_time_and_curated_scope() -> TestResult {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../preservation/corpus/libraries/schema018/history.json"
    ))?;
    let operations = fixture["operations"]
        .as_array()
        .ok_or("missing history operations")?;
    assert_eq!(operations.len(), 3);
    let mut states = Vec::new();
    for record in operations {
        let identity = StoredOperationIdentity {
            id: record["id"].as_str().ok_or("missing ID")?.into(),
            created_at: record["createdAt"]
                .as_str()
                .ok_or("missing createdAt")?
                .into(),
            started_at: record["startedAt"].as_str().map(str::to_owned),
            finished_at: record["finishedAt"].as_str().map(str::to_owned),
        };
        assert_eq!(identity.created_at, "2000-01-01T00:00:00Z");
        let state = decode_state(record["status"].as_str().ok_or("missing status")?)?;
        states.push(state);
        if record["kind"] == "export" {
            assert_eq!(decode_kind("export")?, OperationKind::Export);
            let scope = ExportSourceType::from_persisted(
                record["exportSourceType"]
                    .as_str()
                    .ok_or("missing export scope")?,
            )?;
            assert_eq!(scope, ExportSourceType::CuratedShelf);
            assert_eq!(scope.group_kind(), Some(ExportGroupKind::CuratedShelf));
            assert!(record.get("sourceCollectionIds").is_none());
            assert_eq!(record["sourceShelfIds"], serde_json::json!([1]));
        } else {
            assert_eq!(
                decode_kind(record["kind"].as_str().ok_or("missing kind")?)?,
                OperationKind::Import
            );
            assert!(record.get("exportSourceType").is_none());
            assert_eq!(identity.started_at.as_deref(), Some("2000-01-01 00:00:00"));
        }
        for entry in record["items"].as_array().ok_or("missing items")? {
            let state =
                ItemState::from_persisted(entry["status"].as_str().ok_or("missing item status")?)?;
            if state == ItemState::Succeeded {
                assert_eq!(entry["outcomeCode"], "imported");
            }
        }
    }
    assert_eq!(
        states,
        [
            OperationState::Queued,
            OperationState::Running,
            OperationState::Succeeded
        ]
    );
    Ok(())
}

#[test]
fn op6_admitted_zero_item_operation_interrupts_and_finishes_without_rewriting_scope() -> TestResult
{
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../preservation/corpus/databases/prefix-012.json"
    ))?;
    for phase in ["before", "afterGoOpen"] {
        let tables = &fixture[phase]["tables"];
        let row = tables["file_operations"]
            .as_array()
            .and_then(|rows| rows.first())
            .ok_or("missing admitted legacy operation")?;
        let original = row.clone();
        assert_eq!(row["id"], "80000000-0000-4000-8000-000000000001");
        assert_eq!(row["kind"], "export");
        let counts = Counts {
            completed: row["completed_items"]
                .as_i64()
                .ok_or("missing completed count")?,
            total: row["total_items"].as_i64().ok_or("missing total count")?,
        };
        assert_eq!(
            counts,
            Counts {
                completed: 0,
                total: 0
            }
        );
        assert_eq!(tables["file_operation_items"], serde_json::json!([]));
        counts.validate()?;
        let scope = StoredExportScope {
            discriminator: row["export_source_type"].as_str().map(str::to_owned),
            group_ids: row["export_source_shelf_ids"].as_str().map(str::to_owned),
            group_names: row["export_source_shelf_names"].as_str().map(str::to_owned),
        };
        assert_eq!(scope.source_type()?, Some(ExportSourceType::Shelf));
        assert_eq!(scope.group_ids.as_deref(), Some("[1]"));
        assert_eq!(scope.group_names.as_deref(), Some("[\"Legacy moon\"]"));
        for parent in [OperationState::Queued, OperationState::Running] {
            let repaired = interrupt(
                parent,
                counts,
                &[],
                &outcome("interrupted", "Operation was interrupted."),
            )?;
            assert_eq!(repaired.state, OperationState::Interrupted);
            assert_eq!(repaired.counts, counts);
            assert!(repaired.items.is_empty());
            for target in [
                OperationState::Succeeded,
                OperationState::PartiallySucceeded,
                OperationState::Failed,
                OperationState::Cancelled,
            ] {
                assert_eq!(finish(parent, counts, target)?, target);
            }
            assert_eq!(
                complete_item(parent, ItemState::Pending, counts, ItemState::Succeeded),
                Err(OperationPolicyError::InvalidCounts)
            );
        }
        assert_eq!(*row, original);
        for key in ["started_at", "finished_at", "error_code", "error_message"] {
            assert!(row[key].is_null());
        }
    }
    for kind in [
        OperationKind::Import,
        OperationKind::MetadataSync,
        OperationKind::Export,
    ] {
        for total in [0, -1] {
            assert_eq!(
                Counts::for_new_work(total),
                Err(OperationPolicyError::InvalidCounts)
            );
            assert_eq!(
                CompletionAggregate::new(kind, total),
                Err(OperationPolicyError::InvalidCounts)
            );
        }
    }
    for total in [1, i64::MAX] {
        assert_eq!(
            Counts::for_new_work(total)?,
            Counts {
                completed: 0,
                total
            }
        );
    }
    for counts in [
        Counts {
            completed: -1,
            total: 0,
        },
        Counts {
            completed: 1,
            total: 0,
        },
        Counts {
            completed: 0,
            total: -1,
        },
    ] {
        assert_eq!(counts.validate(), Err(OperationPolicyError::InvalidCounts));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn op6_raw_carriers_preserve_arbitrary_text_and_nullness(
        id in any::<String>(), timestamp in any::<String>(), code in proptest::option::of(any::<String>()),
        names in proptest::option::of(any::<String>())
    ) {
        let identity = StoredOperationIdentity { id: id.clone(), created_at: timestamp.clone(), started_at: None, finished_at: Some(String::new()) };
        let before = identity.clone();
        let _projection = identity.typed_id();
        prop_assert_eq!(&identity, &before);
        prop_assert_eq!(identity.history_key(), HistoryKey { created_at: timestamp, id });
        let raw = RawOutcome { code: code.clone(), message: names.clone() };
        prop_assert_eq!(raw.code, code);
        prop_assert_eq!(raw.message, names);
        prop_assert_eq!(identity.finished_at, Some(String::new()));
    }
    #[test]
    fn op6_progress_acceptance_is_exactly_running_monotone_in_bounds(
        current in 0_i64..10000, total in 0_i64..10000, next in -1_i64..10001,
        state_index in 0_usize..7
    ) {
        let state = ITEMS[state_index];
        let result = Progress { completed: current, total }.advance(state, next);
        prop_assert_eq!(result.is_ok(), state == ItemState::Running && current <= next && next <= total);
        if let Ok(progress) = result { prop_assert_eq!(progress, Progress { completed: next, total }); }
    }
    #[test]
    fn op6_byte_total_matches_wide_sum_or_returns_overflow(values in proptest::collection::vec(0_i64..=i64::MAX, 0..30)) {
        let wide: i128 = values.iter().map(|value| i128::from(*value)).sum();
        let result = checked_byte_total(&values);
        if wide > i128::from(i64::MAX) { prop_assert_eq!(result, Err(OperationPolicyError::ByteTotalOverflow)); }
        else { prop_assert_eq!(result, Ok(wide as i64)); }
    }
}
