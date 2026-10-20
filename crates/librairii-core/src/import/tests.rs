use super::*;

fn reported_outcome(code: OutcomeCode) -> ReportedOutcome {
    ReportedOutcome {
        code,
        raw_uuid: b"{00112233-4455-4677-8899-AABBCCDDEEFF}".to_vec(),
        story_id: Some(71),
        archive_id: Some(89),
        existing_story_id: Some(71),
        checksum: Some(b"ABraw-checksum".to_vec()),
    }
}

#[test]
fn reported_import_and_failure_codes_match_frozen_source_without_unknown_fallback() {
    assert_eq!(
        OutcomeCode::ALL
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>(),
        ["imported", "duplicate_checksum", "uuid_conflict"]
    );
    assert_eq!(
        ImportErrorCode::ALL
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>(),
        [
            "stage_failed",
            "lookup_failed",
            "inspection_failed",
            "publish_failed",
            "artwork_failed",
            "catalog_failed",
            "compensation_failed",
            "cleanup_failed",
            "cancelled"
        ]
    );
    for &code in OutcomeCode::ALL {
        assert_eq!(OutcomeCode::from_code(code.as_str()), Some(code));
    }
    for &code in ImportErrorCode::ALL {
        assert_eq!(ImportErrorCode::from_code(code.as_str()), Some(code));
    }
    assert_eq!(OutcomeCode::from_code("Imported"), None);
    assert_eq!(ImportErrorCode::from_code("commit_uncertain"), None);
}

#[test]
fn committed_facts_and_ids_survive_late_cancel_cleanup_and_reporting_errors() {
    let outcome = reported_outcome(OutcomeCode::Imported);
    let mut report = ReportedExecution::from_supplied_observations(
        Some(outcome.clone()),
        CommitObservation::Committed,
    );
    for (phase, code) in [
        (ProblemPhase::Primary, ImportErrorCode::Cancelled),
        (ProblemPhase::StagingCleanup, ImportErrorCode::Cleanup),
        (ProblemPhase::Reporting, ImportErrorCode::Catalog),
    ] {
        report.record_problem(ReportedProblem {
            phase,
            cause: FailureCause::Import {
                code,
                cause: Some(Box::new(FailureCause::Message("later failure".to_owned()))),
            },
        });
    }
    assert_eq!(report.outcome(), Some(&outcome));
    assert_eq!(report.commit(), CommitObservation::Committed);
    assert_eq!(report.problems().len(), 3);
    assert_eq!(report.problems()[2].phase, ProblemPhase::Reporting);
    // Reporting data cannot release a real guard/lease or compensate bytes.
}

#[test]
fn uncertain_report_does_not_invent_ids_or_noncommit_from_error() {
    let mut report =
        ReportedExecution::from_supplied_observations(None, CommitObservation::Uncertain);
    report.record_problem(ReportedProblem {
        phase: ProblemPhase::Primary,
        cause: FailureCause::Import {
            code: ImportErrorCode::Catalog,
            cause: Some(Box::new(FailureCause::Message(
                "commit acknowledgement lost".to_owned(),
            ))),
        },
    });
    report.record_problem(ReportedProblem {
        phase: ProblemPhase::Reporting,
        cause: FailureCause::Message("reporting failed independently".to_owned()),
    });
    assert_eq!(report.outcome(), None);
    assert_eq!(report.commit(), CommitObservation::Uncertain);
    assert_eq!(report.problems().len(), 2);
    let absent =
        ReportedExecution::from_supplied_observations(None, CommitObservation::NotReported);
    let noncommit =
        ReportedExecution::from_supplied_observations(None, CommitObservation::ProvenNotCommitted);
    assert_ne!(absent, noncommit);
    assert_ne!(absent.commit(), report.commit());
}

#[test]
fn duplicate_conflict_facts_raw_fields_and_independent_compensation_error_remain_owned() {
    for code in [OutcomeCode::DuplicateChecksum, OutcomeCode::UuidConflict] {
        let fact = reported_outcome(code);
        let mut report = ReportedExecution::from_supplied_observations(
            Some(fact.clone()),
            CommitObservation::NotReported,
        );
        report.record_problem(ReportedProblem {
            phase: ProblemPhase::StagingCleanup,
            cause: FailureCause::Import {
                code: ImportErrorCode::Cleanup,
                cause: None,
            },
        });
        report.record_problem(ReportedProblem {
            phase: ProblemPhase::Compensation,
            cause: FailureCause::Message("independent reported recovery problem".to_owned()),
        });
        assert_eq!(report.outcome(), Some(&fact));
        assert_eq!(report.commit(), CommitObservation::NotReported);
        assert_eq!(report.problems().len(), 2);
    }
    // Presence stays explicit; this is unverified data, not positive-ID/hash validation.
    let mut present = reported_outcome(OutcomeCode::Imported);
    present.story_id = Some(0);
    present.checksum = Some(Vec::new());
    let mut absent = present.clone();
    absent.story_id = None;
    absent.checksum = None;
    assert_ne!(present, absent);
}

#[test]
fn nested_joined_causes_preserve_first_validation_order_and_optional_empty_data() {
    let first = ValidationError::at(ValidationCode::UnsafePath, b"../raw").caused_by("");
    let second = ValidationError::new(ValidationCode::MissingEntry);
    let nested = FailureCause::Import {
        code: ImportErrorCode::Inspection,
        cause: Some(Box::new(FailureCause::Inspection(first.clone()))),
    };
    let joined = FailureCause::join([
        None,
        Some(nested.clone()),
        Some(FailureCause::Inspection(second.clone())),
        Some(FailureCause::Message(String::new())),
    ]);
    let expected = FailureCause::Joined(vec![
        nested,
        FailureCause::Inspection(second),
        FailureCause::Message(String::new()),
    ]);
    assert_eq!(joined, Some(expected));
    assert_eq!(
        joined.as_ref().and_then(FailureCause::first_validation),
        Some(&first)
    );
    assert_eq!(FailureCause::join([None, None]), None);
    assert_eq!(
        FailureCause::join([Some(FailureCause::Message(String::new()))]),
        Some(FailureCause::Joined(vec![FailureCause::Message(
            String::new()
        )]))
    );
    assert_ne!(
        FailureCause::Import {
            code: ImportErrorCode::Cleanup,
            cause: None
        },
        FailureCause::Import {
            code: ImportErrorCode::Cleanup,
            cause: Some(Box::new(FailureCause::Message(String::new())))
        }
    );
}

#[test]
fn import_uuid_retains_legacy_spelling_without_changing_shared_identity_parsing()
-> Result<(), ValidationError> {
    let canonical = "00112233-4455-4677-8899-aabbccddeeff";
    for text in [
        canonical,
        "00112233445546778899AABBCCDDEEFF",
        "{00112233-4455-4677-8899-AABBCCDDEEFF}",
        "URN:UUID:00112233-4455-4677-8899-AABBCCDDEEFF",
        "!00112233-4455-4677-8899-aabbccddeeff?",
    ] {
        let identity = ImportIdentity::parse_text(text.as_bytes())?;
        assert_eq!(identity.canonical(), canonical);
        assert_eq!(identity.raw(), &UuidSource::Text(text.as_bytes().to_vec()));
    }
    let raw = [b"\xff".as_slice(), canonical.as_bytes(), b"\xfe"].concat();
    let identity = ImportIdentity::parse_text(&raw)?;
    assert_eq!(identity.raw(), &UuidSource::Text(raw));
    assert_eq!(identity.canonical(), canonical);
    // This matches google/uuid Parse's middle36 rule. An archive entry must
    // separately pass UTF8/path policy; no raw header/filename evidence implied.
    assert_eq!(crate::StoryUuid::from_bytes([0; 16]).as_bytes(), &[0; 16]);
    assert_eq!(
        ImportIdentity::from_plain_bytes(&[0; 16]).map_err(|e| e.code),
        Err(ValidationCode::InvalidUuid)
    );
    Ok(())
}

#[test]
fn malformed_or_nil_uuid_forms_fail_at_import_boundary() {
    for text in [
        "",
        "00112233445546778899aabbccddeef",
        "00112233_4455-4677-8899-aabbccddeeff",
        "00112233-4455-4677-8899-aabbccddeefg",
        "xxx:uuid:00112233-4455-4677-8899-aabbccddeeff",
        "00000000-0000-0000-0000-000000000000",
        "00000000000000000000000000000000",
        "{00000000-0000-0000-0000-000000000000}",
        "urn:uuid:00000000-0000-0000-0000-000000000000",
    ] {
        assert_eq!(
            ImportIdentity::parse_text(text.as_bytes()).map_err(|e| e.code),
            Err(ValidationCode::InvalidUuid),
            "{text}"
        );
    }
    for length in [0, 15, 17] {
        let data = vec![1; length];
        assert_eq!(
            ImportIdentity::from_plain_bytes(&data).map_err(|e| (e.code, e.entry)),
            Err((ValidationCode::InvalidUuid, Some(b"uuid.bin".to_vec())))
        );
    }
}

fn inventory_with_files(names: &[String]) -> Result<Inventory, ValidationError> {
    let mut builder = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    for name in names {
        builder.push(header(name.as_bytes(), 1, 1))?;
    }
    builder.finish()
}

#[test]
fn plain_required_entry_order_assets_and_exact_uuid_payload_are_separate()
-> Result<(), ValidationError> {
    let names: Vec<String> = [
        "uuid.bin", "ni", "li.plain", "ri.plain", "si.plain", "rf/sound", "sf/image",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    validate_plain_entries(&inventory_with_files(&names)?)?;
    for (removed, expected) in [
        (0, "uuid.bin"),
        (1, "ni"),
        (2, "li.plain"),
        (3, "ri.plain"),
        (4, "si.plain"),
        (5, "rf/"),
        (6, "sf/"),
    ] {
        let selected: Vec<_> = names
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != removed)
            .map(|(_, n)| n.clone())
            .collect();
        assert_eq!(
            validate_plain_entries(&inventory_with_files(&selected)?)
                .map_err(|e| (e.code, e.entry)),
            Err((
                ValidationCode::MissingEntry,
                Some(expected.as_bytes().to_vec())
            ))
        );
    }
    let bytes = [
        0, 17, 34, 51, 68, 85, 70, 119, 136, 153, 170, 187, 204, 221, 238, 255,
    ];
    let identity = ImportIdentity::from_plain_bytes(&bytes)?;
    assert_eq!(identity.raw(), &UuidSource::PlainBytes(bytes));
    assert_eq!(identity.canonical(), "00112233-4455-4677-8899-aabbccddeeff");
    Ok(())
}

#[test]
fn uuid_root_detection_uses_raw_root_count_before_required_entries() -> Result<(), ValidationError>
{
    let raw = "00112233445546778899AABBCCDDEEFF";
    let names: Vec<_> = ["ni", "li", "ri", "si", "rf/sound", "sf/image"]
        .into_iter()
        .map(|n| format!("{raw}/{n}"))
        .collect();
    let root = validate_uuid_root_entries(&inventory_with_files(&names)?)?;
    assert_eq!(root.raw_name, raw);
    assert_eq!(
        root.identity.raw(),
        &UuidSource::Text(raw.as_bytes().to_vec())
    );
    assert_eq!(
        root.identity.canonical(),
        "00112233-4455-4677-8899-aabbccddeeff"
    );
    let two = [
        format!("{raw}/ni"),
        "00112233445546778899aabbccddeeff/ni".to_owned(),
    ];
    assert_eq!(
        validate_uuid_root_entries(&inventory_with_files(&two)?).map_err(|e| e.code),
        Err(ValidationCode::AmbiguousStructure)
    );
    let invalid = [
        "no-uuid/ni".to_owned(),
        "00000000000000000000000000000000/ni".to_owned(),
    ];
    assert_eq!(
        validate_uuid_root_entries(&inventory_with_files(&invalid)?).map_err(|e| e.code),
        Err(ValidationCode::InvalidUuid)
    );
    Ok(())
}

#[test]
fn uuid_pack_missing_entries_and_assets_preserve_exact_root_spelling() -> Result<(), ValidationError>
{
    let raw = "URN:UUID:00112233-4455-4677-8899-AABBCCDDEEFF";
    let leaves = ["ni", "li", "ri", "si", "rf/sound", "sf/image"];
    for (removed, missing) in [
        (0, "ni"),
        (1, "li"),
        (2, "ri"),
        (3, "si"),
        (4, "rf/"),
        (5, "sf/"),
    ] {
        let names: Vec<_> = leaves
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != removed)
            .map(|(_, n)| format!("{raw}/{n}"))
            .collect();
        assert_eq!(
            validate_uuid_root_entries(&inventory_with_files(&names)?)
                .map_err(|e| (e.code, e.entry)),
            Err((
                ValidationCode::MissingEntry,
                Some(format!("{raw}/{missing}").into_bytes())
            ))
        );
    }
    Ok(())
}

#[test]
fn candidate_bytes_preserve_source_name_uuid_and_absent_vs_empty_payload()
-> Result<(), ValidationError> {
    let raw = "{00112233-4455-4677-8899-AABBCCDDEEFF}";
    let absent = CandidateData {
        original_filename: b"\xfforiginal.PK".to_vec(),
        identity: ImportIdentity::parse_text(raw.as_bytes())?,
        format: ArchiveFormat::V1Pk,
        metadata_payload: None,
    };
    let mut present = absent.clone();
    present.metadata_payload = Some(Vec::new());
    assert_ne!(absent, present);
    assert_eq!(present.original_filename, b"\xfforiginal.PK");
    assert_eq!(
        present.identity.raw(),
        &UuidSource::Text(raw.as_bytes().to_vec())
    );
    // No source hash, decoder proof, DB IDs, owned stage or receipt is minted.
    Ok(())
}

#[test]
fn dispatch_preserves_suffix_and_content_precedence() -> Result<(), ValidationError> {
    use ArchiveFormat::{GenericPk, PlainPk, StudioZip, V1Pk, V2Pk};
    use ContainerKind::Zip;
    use PackStructure::*;
    let cases = [
        ("story.plain.pk", Zip, false, false, PlainPk, Plain),
        ("story.v1.pk", Zip, false, false, V1Pk, UuidRoot),
        ("story.V2.PK", Zip, false, false, V2Pk, UuidRoot),
        ("story.pk", Zip, false, false, GenericPk, UuidRoot),
        ("story.zip", Zip, false, false, ArchiveFormat::Zip, UuidRoot),
        ("story.v1.pk", Zip, false, true, PlainPk, Plain),
        ("story.zip", Zip, false, true, PlainPk, Plain),
        ("story.zip", Zip, true, true, StudioZip, Studio),
        (
            "story.7Z",
            ContainerKind::SevenZip,
            false,
            true,
            ArchiveFormat::SevenZip,
            UuidRoot,
        ),
        (
            "story.7z",
            ContainerKind::SevenZip,
            true,
            true,
            ArchiveFormat::SevenZip,
            Studio,
        ),
    ];
    for (name, container, has_story_json, has_uuid_bin, format, structure) in cases {
        let plan = dispatch(
            name.as_bytes(),
            ContentFacts {
                container,
                has_story_json,
                has_uuid_bin,
            },
        )?;
        assert_eq!((plan.format, plan.structure), (format, structure), "{name}");
        assert_eq!(
            plan.integrity,
            if container == ContainerKind::SevenZip {
                IntegrityRequirement::SelectedSevenZipPayloads
            } else {
                IntegrityRequirement::AllZipPayloads
            }
        );
    }
    Ok(())
}

#[test]
fn misleading_suffix_container_and_disguised_studio_fail_distinctly() {
    for name in ["story.pk", "story.v1.pk", "story.plain.pk"] {
        assert_eq!(
            dispatch(
                name.as_bytes(),
                ContentFacts {
                    container: ContainerKind::Zip,
                    has_story_json: true,
                    has_uuid_bin: true,
                }
            )
            .map_err(|e| e.code),
            Err(ValidationCode::UnsupportedFormat)
        );
    }
    for (name, kind, expected) in [
        (
            "story.txt",
            ContainerKind::Zip,
            ValidationCode::UnsupportedFormat,
        ),
        (
            "story.zip ",
            ContainerKind::Zip,
            ValidationCode::UnsupportedFormat,
        ),
        (
            "story.pk",
            ContainerKind::SevenZip,
            ValidationCode::InvalidContainer,
        ),
        (
            "story.7z",
            ContainerKind::Zip,
            ValidationCode::InvalidContainer,
        ),
        (
            "story.zip",
            ContainerKind::Other,
            ValidationCode::InvalidContainer,
        ),
    ] {
        assert_eq!(
            dispatch(
                name.as_bytes(),
                ContentFacts {
                    container: kind,
                    has_story_json: false,
                    has_uuid_bin: false,
                }
            )
            .map_err(|e| e.code),
            Err(expected)
        );
    }
}

#[test]
fn suffix_matching_is_simple_lower_not_search_folding_or_lossy_name_conversion()
-> Result<(), ValidationError> {
    let facts = ContentFacts {
        container: ContainerKind::Zip,
        has_story_json: false,
        has_uuid_bin: false,
    };
    assert_eq!(
        dispatch("Été 日本語.pK".as_bytes(), facts)?.format,
        ArchiveFormat::GenericPk
    );
    assert_eq!(
        dispatch("story.zİp".as_bytes(), facts)?.format,
        ArchiveFormat::Zip
    );
    assert_eq!(
        dispatch(b"\xffraw.PK", facts)?.format,
        ArchiveFormat::GenericPk
    );
    assert!(dispatch("story.ｐｋ".as_bytes(), facts).is_err());
    Ok(())
}

#[test]
fn frozen_codes_are_exact_and_errors_preserve_presence_and_raw_entry() {
    assert_eq!(ArchiveFormat::ALL.len(), 7);
    assert_eq!(ValidationCode::ALL.len(), 16);
    for &format in ArchiveFormat::ALL {
        assert_eq!(ArchiveFormat::from_code(format.as_str()), Some(format));
    }
    for &code in ValidationCode::ALL {
        assert_eq!(ValidationCode::from_code(code.as_str()), Some(code));
    }
    assert_eq!(ArchiveFormat::from_code("ZIP"), None);
    assert_eq!(ValidationCode::from_code("new_unknown_code"), None);
    let absent = ValidationError::new(ValidationCode::UnsafePath);
    let present = ValidationError::at(ValidationCode::UnsafePath, b"\xff\x00").caused_by("");
    assert_eq!(absent.entry, None);
    assert_eq!(present.entry.as_deref(), Some(b"\xff\x00".as_slice()));
    assert_eq!(present.cause.as_deref(), Some(""));
    assert_eq!(present.to_string(), "unsafe_path validation failed");
}

fn header(name: &[u8], compressed_bytes: i64, uncompressed_bytes: i64) -> EntryInfo<'_> {
    EntryInfo {
        name,
        compressed_bytes,
        uncompressed_bytes,
        is_directory: false,
        is_link: false,
    }
}

#[test]
fn paths_use_go_lexical_keys_preserving_raw_bytes_and_unicode_forms() -> Result<(), ValidationError>
{
    let limits = Limits::default();
    for (raw, key) in [
        ("a/../ni", "ni"),
        ("./rf//sound/", "rf/sound"),
        ("a/./b/../c", "a/c"),
        ("Été 日本語.bin", "Été 日本語.bin"),
        (
            "E\u{301}te\u{301} 日本語.bin",
            "E\u{301}te\u{301} 日本語.bin",
        ),
    ] {
        assert_eq!(validate_entry_name(raw.as_bytes(), limits)?, key);
    }
    for raw in [
        b"".as_slice(),
        b".",
        b"a/..",
        b"../ni",
        b"a/../../ni",
        b"/ni",
        b"//ni",
        b"C:ni",
        b"D:/ni",
        b"rf\\ni",
        b"ni\0",
        b"\xffni",
    ] {
        let error = validate_entry_name(raw, limits).map_err(|e| (e.code, e.entry));
        assert_eq!(error, Err((ValidationCode::UnsafePath, Some(raw.to_vec()))));
    }
    Ok(())
}

#[test]
fn duplicate_lexical_aliases_are_terminal_and_not_unicode_composed() -> Result<(), ValidationError>
{
    let mut builder = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    builder.push(header(b"rf/./voice", 1, 1))?;
    let duplicate = builder.push(header(b"rf/a/../voice", 1, 1));
    assert_eq!(
        duplicate.as_ref().map_err(|e| e.code),
        Err(ValidationCode::AmbiguousStructure)
    );
    assert_eq!(builder.push(header(b"other", 1, 1)), duplicate);
    assert_eq!(
        builder.finish().map_err(|e| e.code),
        Err(ValidationCode::AmbiguousStructure)
    );

    let mut builder = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    let nfc = "Été 日本語.bin".as_bytes();
    let nfd = "E\u{301}te\u{301} 日本語.bin".as_bytes();
    builder.push(header(nfc, 1, 1))?;
    builder.push(header(nfd, 1, 1))?;
    let inventory = builder.finish()?;
    assert_eq!(inventory.usage().entries, 2);
    assert_eq!(
        inventory
            .entry("Été 日本語.bin")
            .map(|e| e.raw_name.as_slice()),
        Some(nfc)
    );
    assert_eq!(
        inventory
            .entry("E\u{301}te\u{301} 日本語.bin")
            .map(|e| e.raw_name.as_slice()),
        Some(nfd)
    );
    // Synthetic policy inputs only: these are not raw7z/Go filename observations.
    Ok(())
}

#[test]
fn directories_count_but_do_not_satisfy_files_or_assets() -> Result<(), ValidationError> {
    let mut builder = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    builder.push(EntryInfo {
        is_directory: true,
        ..header(b"story.json/", 0, 0)
    })?;
    builder.push(EntryInfo {
        is_directory: true,
        ..header(b"rf/", 0, 0)
    })?;
    builder.push(header(b"rfish/other", 1, 1))?;
    let inventory = builder.finish()?;
    assert!(!inventory.has_file("story.json"));
    assert!(!inventory.has_file_below("rf"));
    assert_eq!(inventory.file_names().collect::<Vec<_>>(), ["rfish/other"]);
    assert_eq!(inventory.usage().entries, 3);
    Ok(())
}

#[test]
fn unsafe_link_negative_duplicate_and_nested_checks_keep_source_priority()
-> Result<(), ValidationError> {
    let mut budget = Budget::new(Limits::default())?;
    assert_eq!(
        budget
            .account(EntryInfo {
                is_link: true,
                ..header(b"../bad", -1, -1)
            })
            .map_err(|e| e.code),
        Err(ValidationCode::UnsafePath)
    );
    assert_eq!(
        budget
            .account(EntryInfo {
                is_link: true,
                ..header(b"safe", -1, -1)
            })
            .map_err(|e| e.code),
        Err(ValidationCode::LinkEntry)
    );
    assert_eq!(
        budget.account(header(b"safe", -1, 0)).map_err(|e| e.code),
        Err(ValidationCode::InvalidContainer)
    );
    for name in [
        "nested.zip",
        "nested.7Z",
        "nested.v1.pk",
        "nested.plain.pk",
        "nested.pK",
    ] {
        let mut b = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
        assert_eq!(
            b.push(header(name.as_bytes(), 1, 1)).map_err(|e| e.code),
            Err(ValidationCode::NestedArchive)
        );
    }
    let mut b = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    b.push(EntryInfo {
        is_directory: true,
        ..header(b"nested.zip/", 0, 0)
    })?;
    assert_eq!(
        b.push(header(b"nested.zip", 1, 1)).map_err(|e| e.code),
        Err(ValidationCode::AmbiguousStructure)
    );
    Ok(())
}

#[test]
fn invalid_limits_and_unsigned_or_aggregate_overflow_refuse_without_wrapping()
-> Result<(), ValidationError> {
    for limits in [
        Limits {
            max_path_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_path_depth: 0,
            ..Limits::default()
        },
        Limits {
            max_entries: 0,
            ..Limits::default()
        },
        Limits {
            max_expanded_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_metadata_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_artwork_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_compression_ratio: 0.99,
            ..Limits::default()
        },
        Limits {
            max_compression_ratio: f64::NAN,
            ..Limits::default()
        },
        Limits {
            max_compression_ratio: f64::INFINITY,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            limits.validate().map_err(|e| e.code),
            Err(ValidationCode::InvalidLimits)
        );
    }
    for (compressed, expanded) in [(u64::MAX, 0), (0, u64::MAX)] {
        assert_eq!(
            EntryInfo::from_unsigned(b"entry", compressed, expanded, false, false)
                .map_err(|e| e.code),
            Err(ValidationCode::InvalidContainer)
        );
    }
    let mut b = Budget::new(Limits {
        max_expanded_bytes: i64::MAX,
        ..Limits::default()
    })?;
    b.account(header(b"first", i64::MAX, i64::MAX))?;
    let previous = b.usage();
    assert_eq!(
        b.account(header(b"second", 0, 1)).map_err(|e| e.code),
        Err(ValidationCode::ExpandedSizeLimit)
    );
    assert_eq!(b.usage(), previous);
    assert_eq!(
        b.account(header(b"second", 1, 0)).map_err(|e| e.code),
        Err(ValidationCode::InvalidContainer)
    );
    assert_eq!(b.usage(), previous);
    Ok(())
}

#[test]
fn zip_entry_and_sevenzip_whole_container_ratios_are_distinct() -> Result<(), ValidationError> {
    let mut zip = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    assert_eq!(
        zip.push(header(b"ni", 1, 201)).map_err(|e| e.code),
        Err(ValidationCode::CompressionRatioLimit)
    );
    let mut seven = InventoryBuilder::new(
        Limits::default(),
        CompressionAccounting::SevenZipContainer { bytes: 2 },
    )?;
    seven.push(header(b"ni", 0, 201))?;
    assert_eq!(seven.finish()?.usage().expanded_bytes, 201);
    let mut seven = InventoryBuilder::new(
        Limits::default(),
        CompressionAccounting::SevenZipContainer { bytes: 1 },
    )?;
    seven.push(header(b"ni", 0, 201))?;
    let error = seven.finish().map_err(|e| (e.code, e.entry));
    assert_eq!(error, Err((ValidationCode::CompressionRatioLimit, None)));
    let mut zero = Budget::new(Limits::default())?;
    zero.account(header(b"empty", 0, 0))?;
    zero.validate_compression(0)?;
    zero.account(header(b"zero-packed", 0, 200))?;
    assert_eq!(
        zero.account(header(b"over", 0, 1)).map_err(|e| e.code),
        Err(ValidationCode::CompressionRatioLimit)
    );
    Ok(())
}

#[test]
fn retained_header_limits_match_all_frozen_expected_codes() -> Result<(), Box<dyn std::error::Error>>
{
    // The retained primitive cases are data only: no archive decoder is run.
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../preservation/corpus/limits/archive-headers.json"
    ))?;
    let cases = cases.as_array().ok_or("expected array")?;
    assert_eq!(cases.len(), 21);
    for case in cases {
        let l = &case["limits"];
        let integer = |key: &str| l[key].as_i64().ok_or("missing limit integer");
        let limits = Limits {
            max_path_bytes: usize::try_from(integer("MaxPathBytes")?)?,
            max_path_depth: usize::try_from(integer("MaxPathDepth")?)?,
            max_entries: usize::try_from(integer("MaxEntries")?)?,
            max_expanded_bytes: integer("MaxExpandedBytes")?,
            max_compression_ratio: l["MaxCompressionRatio"].as_f64().ok_or("missing ratio")?,
            max_metadata_bytes: integer("MaxMetadataBytes")?,
            max_artwork_bytes: integer("MaxArtworkBytes")?,
        };
        let mut budget = Budget::new(limits)?;
        let mut actual = String::new();
        'repeat: for _ in 0..case["repeat"].as_u64().ok_or("missing repeat")? {
            for e in case["entries"].as_array().ok_or("missing entries")? {
                let entry = EntryInfo {
                    name: e["Name"].as_str().ok_or("missing name")?.as_bytes(),
                    compressed_bytes: e["CompressedBytes"].as_i64().ok_or("missing size")?,
                    uncompressed_bytes: e["UncompressedBytes"].as_i64().ok_or("missing size")?,
                    is_directory: e["IsDirectory"].as_bool().ok_or("missing directory flag")?,
                    is_link: e["IsLink"].as_bool().ok_or("missing link flag")?,
                };
                if let Err(error) = budget.account(entry) {
                    actual = error.code.as_str().to_owned();
                    break 'repeat;
                }
            }
        }
        assert_eq!(
            actual,
            case["error"].as_str().ok_or("missing error")?,
            "{}",
            case["id"]
        );
    }
    Ok(())
}
