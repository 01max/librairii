use super::*;

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
