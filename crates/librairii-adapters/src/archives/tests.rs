use super::*;
use librairii_core::import::{ArchiveFormat, PackStructure};
use std::{fs, io::Cursor, path::PathBuf};
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
fn fixture(name: &str) -> io::Result<Vec<u8>> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../preservation/corpus/archives")
            .join(name),
    )
}
fn open<'a>(bytes: &'a [u8], name: &[u8]) -> Result<Archive<Cursor<&'a [u8]>>> {
    Archive::open(
        Cursor::new(bytes),
        name,
        Limits::default(),
        &Cancellation::default(),
    )
}
fn rejected(bytes: &[u8], name: &[u8], code: ValidationCode) {
    let result = open(bytes, name);
    assert!(
        matches!(result, Err(ArchiveError::Validation(ValidationError { code: actual, .. })) if actual == code),
        "{result:?}"
    );
}
#[test]
fn actual_zip_and_seven_zip_dispatch_preserves_suffix_content_combinations() -> TestResult {
    for (source, filename, format, structure) in [
        (
            "clockwork-forest.plain.pk",
            "story.PLAIN.PK",
            ArchiveFormat::PlainPk,
            PackStructure::Plain,
        ),
        (
            "clockwork-forest.plain.pk",
            "story.v1.pk",
            ArchiveFormat::PlainPk,
            PackStructure::Plain,
        ),
        (
            "clockwork-forest.pk",
            "story.v1.pk",
            ArchiveFormat::V1Pk,
            PackStructure::UuidRoot,
        ),
        (
            "clockwork-forest.pk",
            "story.v2.pk",
            ArchiveFormat::V2Pk,
            PackStructure::UuidRoot,
        ),
        (
            "clockwork-forest.pk",
            "story.pk",
            ArchiveFormat::GenericPk,
            PackStructure::UuidRoot,
        ),
        (
            "clockwork-forest.pk",
            "story.zip",
            ArchiveFormat::Zip,
            PackStructure::UuidRoot,
        ),
        (
            "clockwork-forest-studio.zip",
            "story.ZIP",
            ArchiveFormat::StudioZip,
            PackStructure::Studio,
        ),
        (
            "generic.7z",
            "story.7Z",
            ArchiveFormat::SevenZip,
            PackStructure::UuidRoot,
        ),
        (
            "studio.7z",
            "story.7z",
            ArchiveFormat::SevenZip,
            PackStructure::Studio,
        ),
    ] {
        let bytes = fixture(source)?;
        let archive = open(&bytes, filename.as_bytes())?;
        assert_eq!(archive.plan().format, format, "{filename}");
        assert_eq!(archive.plan().structure, structure, "{filename}");
    }
    Ok(())
}
#[test]
fn actual_studio_zip_disguised_as_every_pack_suffix_is_rejected() -> TestResult {
    let bytes = fixture("clockwork-forest-studio.zip")?;
    for name in [
        "studio.pk",
        "studio.plain.pk",
        "studio.v1.pk",
        "studio.v2.pk",
    ] {
        rejected(&bytes, name.as_bytes(), ValidationCode::UnsupportedFormat);
    }
    Ok(())
}
#[test]
fn actual_container_mismatch_and_unsupported_suffix_do_not_admit_an_archive() -> TestResult {
    let zip = fixture("clockwork-forest.pk")?;
    let seven = fixture("generic.7z")?;
    rejected(&zip, b"story.7z", ValidationCode::InvalidContainer);
    for name in ["story.pk", "story.zip", "story.plain.pk"] {
        rejected(&seven, name.as_bytes(), ValidationCode::InvalidContainer);
    }
    rejected(&zip, b"story.txt", ValidationCode::UnsupportedFormat);
    rejected(
        b"not an archive",
        b"story.pk",
        ValidationCode::InvalidContainer,
    );
    Ok(())
}
#[test]
fn indexed_archive_reads_selected_metadata_without_bulk_extraction() -> TestResult {
    for (source, entry) in [
        ("clockwork-forest.plain.pk", "uuid.bin"),
        ("studio.7z", "story.json"),
    ] {
        let bytes = fixture(source)?;
        let mut archive = open(&bytes, source.as_bytes())?;
        let payload = archive.read_entry(entry, 1024 * 1024, ValidationCode::MetadataLimit)?;
        assert!(!payload.is_empty());
        assert!(matches!(
            archive.read_entry(entry, 1, ValidationCode::MetadataLimit),
            Err(ArchiveError::Validation(ValidationError {
                code: ValidationCode::MetadataLimit,
                ..
            }))
        ));
        assert_eq!(bytes, fixture(source)?);
    }
    Ok(())
}

fn zip_entries(
    entries: &[(&str, &[u8])],
) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut output = Vec::new();
    let mut archive = rawzip::ZipArchiveWriter::new(&mut output);
    for (name, bytes) in entries {
        let (mut entry, config) = archive
            .new_file(rawzip::path::EntryPath::verbatim(name.as_bytes()))
            .compression_method(rawzip::CompressionMethod::STORE)
            .start()?;
        let mut writer = config.wrap(&mut entry);
        writer.write_all(bytes)?;
        let (_, descriptor) = writer.finish()?;
        entry.finish(descriptor)?;
    }
    archive.finish()?;
    Ok(output)
}
#[test]
fn actual_entry_path_and_duplicate_checks_use_preserved_normalized_keys() -> TestResult {
    for (names, code) in [
        (vec!["../outside"], ValidationCode::UnsafePath),
        (vec!["/root"], ValidationCode::UnsafePath),
        (vec!["C:drive"], ValidationCode::UnsafePath),
        (vec!["a\\b"], ValidationCode::UnsafePath),
        (vec!["a", "./a"], ValidationCode::AmbiguousStructure),
        (vec!["a/b", "a//b"], ValidationCode::AmbiguousStructure),
        (vec!["nested.ZIP"], ValidationCode::NestedArchive),
        (vec!["nested.7z"], ValidationCode::NestedArchive),
        (vec!["nested.v1.PK"], ValidationCode::NestedArchive),
    ] {
        let entries: Vec<_> = names
            .into_iter()
            .map(|name| (name, b"data".as_slice()))
            .collect();
        rejected(&zip_entries(&entries)?, b"story.zip", code);
    }
    let long = "x".repeat(513);
    rejected(
        &zip_entries(&[(&long, b"x")])?,
        b"story.zip",
        ValidationCode::UnsafePath,
    );
    let deep = vec!["part"; 17].join("/");
    rejected(
        &zip_entries(&[(&deep, b"x")])?,
        b"story.zip",
        ValidationCode::UnsafePath,
    );
    let mut invalid_utf8 = zip_entries(&[("bad-name", b"x")])?;
    for offset in 0..invalid_utf8.len() - 8 {
        if &invalid_utf8[offset..offset + 8] == b"bad-name" {
            invalid_utf8[offset] = 0xff;
        }
    }
    rejected(&invalid_utf8, b"story.zip", ValidationCode::UnsafePath);
    Ok(())
}
#[test]
fn actual_frozen_archive_rejections_cover_links_nested_duplicates_and_seven_ratio() -> TestResult {
    for (file, code) in [
        (
            "duplicate-path.plain.pk",
            ValidationCode::AmbiguousStructure,
        ),
        ("nested.plain.pk", ValidationCode::NestedArchive),
        ("deep-path.7z", ValidationCode::UnsafePath),
        ("symlink.7z", ValidationCode::LinkEntry),
        ("nested.7z", ValidationCode::NestedArchive),
        ("bomb.7z", ValidationCode::CompressionRatioLimit),
    ] {
        rejected(&fixture(file)?, file.as_bytes(), code);
    }
    Ok(())
}
#[test]
fn entry_and_expansion_limits_are_enforced_from_actual_container_headers() -> TestResult {
    for file in ["clockwork-forest.plain.pk", "generic.7z"] {
        let bytes = fixture(file)?;
        for (limits, code) in [
            (
                Limits {
                    max_entries: 1,
                    ..Default::default()
                },
                ValidationCode::EntryLimit,
            ),
            (
                Limits {
                    max_expanded_bytes: 1,
                    ..Default::default()
                },
                ValidationCode::ExpandedSizeLimit,
            ),
        ] {
            let result = Archive::open(
                Cursor::new(&bytes),
                file.as_bytes(),
                limits,
                &Cancellation::default(),
            );
            assert!(
                matches!(result, Err(ArchiveError::Validation(ValidationError { code: actual, .. })) if actual == code),
                "{file}: {result:?}"
            );
        }
    }
    let bytes = zip_entries(&[("a", b"1"), ("b", b"22")])?;
    let mut archive = Archive::open(
        Cursor::new(bytes),
        b"story.zip",
        Limits {
            max_entries: 2,
            max_expanded_bytes: 3,
            ..Default::default()
        },
        &Cancellation::default(),
    )?;
    assert_eq!(archive.inventory().usage().expanded_bytes, 3);
    assert_eq!(
        archive.read_entry("b", 2, ValidationCode::MetadataLimit)?,
        b"22"
    );
    assert!(matches!(
        archive.read_entry("b", 1, ValidationCode::MetadataLimit),
        Err(ArchiveError::Validation(ValidationError {
            code: ValidationCode::MetadataLimit,
            ..
        }))
    ));
    Ok(())
}
#[test]
fn entry_actual_read_errors_and_corrupt_payload_crc_cannot_return_partial_bytes() -> TestResult {
    let mut bytes = zip_entries(&[("payload", b"unique payload bytes")])?;
    let offset = bytes
        .windows(20)
        .position(|x| x == b"unique payload bytes")
        .ok_or("fixture payload missing")?;
    bytes[offset] ^= 1;
    let mut archive = open(&bytes, b"story.zip")?;
    assert!(
        archive
            .read_entry("payload", 100, ValidationCode::MetadataLimit)
            .is_err()
    );
    let too_wide = Limits {
        max_entries: 10_001,
        ..Default::default()
    };
    assert!(matches!(
        Archive::open(
            Cursor::new(bytes),
            b"story.zip",
            too_wide,
            &Cancellation::default()
        ),
        Err(ArchiveError::Validation(ValidationError {
            code: ValidationCode::InvalidLimits,
            ..
        }))
    ));
    Ok(())
}

#[test]
fn actual_plain_and_uuid_root_packs_match_preserved_identity_and_format() -> TestResult {
    for file in [
        "clockwork-forest.plain.pk",
        "00112233445546778899aabbccddeeff.v1.pk",
        "00112233445546778899aabbccddeeff.v2.pk",
        "clockwork-forest.pk",
        "clockwork-forest.zip",
        "generic.7z",
    ] {
        let bytes = fixture(file)?;
        let mut archive = open(&bytes, file.as_bytes())?;
        assert_eq!(
            archive.inspect_pack_identity()?.canonical(),
            "00112233-4455-4677-8899-aabbccddeeff",
            "{file}"
        );
    }
    Ok(())
}
#[test]
fn actual_plain_uuid_size_nil_and_missing_entry_failures_are_not_filename_identity() -> TestResult {
    for uuid in [vec![0; 16], vec![1; 15], vec![1; 17]] {
        let bytes = zip_entries(&[
            ("uuid.bin", &uuid),
            ("ni", b"n"),
            ("li.plain", b"l"),
            ("ri.plain", b"r"),
            ("si.plain", b"s"),
            ("rf/a", b"r"),
            ("sf/a", b"s"),
        ])?;
        let mut archive = open(&bytes, b"00112233445546778899aabbccddeeff.plain.pk")?;
        assert!(matches!(
            archive.inspect_pack_identity(),
            Err(ArchiveError::Validation(ValidationError {
                code: ValidationCode::InvalidUuid,
                ..
            }))
        ));
    }
    let bytes = zip_entries(&[("uuid.bin", &[1; 16])])?;
    let mut archive = open(&bytes, b"story.v2.pk")?;
    assert_eq!(archive.plan().format, ArchiveFormat::PlainPk);
    assert!(
        matches!(archive.inspect_pack_identity(), Err(ArchiveError::Validation(ValidationError { code: ValidationCode::MissingEntry, entry: Some(entry), .. })) if entry == b"ni")
    );
    Ok(())
}
#[test]
fn actual_uuid_roots_require_one_raw_root_and_all_pack_entries() -> TestResult {
    let root = "00112233-4455-4677-8899-aabbccddeeff";
    for (paths, code) in [
        (
            vec!["not-a-uuid/ni".to_string()],
            ValidationCode::InvalidUuid,
        ),
        (vec![format!("{root}/ni")], ValidationCode::MissingEntry),
        (
            vec![
                format!("{root}/ni"),
                "00112233445546778899aabbccddeeff/ni".to_string(),
            ],
            ValidationCode::AmbiguousStructure,
        ),
    ] {
        let entries: Vec<_> = paths
            .iter()
            .map(|p| (p.as_str(), b"x".as_slice()))
            .collect();
        let bytes = zip_entries(&entries)?;
        let mut archive = open(&bytes, b"story.pk")?;
        assert!(
            matches!(archive.inspect_pack_identity(), Err(ArchiveError::Validation(ValidationError { code: actual, .. })) if actual == code)
        );
    }
    let bytes = fixture("missing.7z")?;
    let mut archive = open(&bytes, b"story.7z")?;
    assert!(matches!(
        archive.inspect_pack_identity(),
        Err(ArchiveError::Validation(ValidationError {
            code: ValidationCode::MissingEntry,
            ..
        }))
    ));
    Ok(())
}

fn png() -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = fixture("clockwork-forest-studio.zip")?;
    Ok(open(&bytes, b"studio.zip")?.read_entry(
        "thumbnail.png",
        10 << 20,
        ValidationCode::ArtworkLimit,
    )?)
}
#[test]
fn actual_studio_zip_and_seven_json_assets_text_and_thumbnail_match_oracle() -> TestResult {
    for file in ["clockwork-forest-studio.zip", "studio.7z"] {
        let bytes = fixture(file)?;
        let mut archive = open(&bytes, file.as_bytes())?;
        let metadata = archive.inspect_studio_metadata()?;
        assert_eq!(
            metadata.identity.canonical(),
            "00112233-4455-4677-8899-aabbccddeeff"
        );
        assert_eq!(metadata.text.title, "The Clockwork Forest");
        assert_eq!(metadata.text.description, "A synthetic STUdio fixture.");
        assert_eq!(metadata.artwork.ok_or("missing thumbnail")?.bytes, png()?);
        assert_eq!(
            metadata.format,
            if file.ends_with(".zip") {
                ArchiveFormat::StudioZip
            } else {
                ArchiveFormat::SevenZip
            }
        );
    }
    Ok(())
}
#[test]
fn actual_studio_json_rejects_strict_type_trailing_and_missing_references() -> TestResult {
    let thumbnail = png()?;
    for (story, code) in [
        (br#"{"uuid":1}"#.as_slice(), ValidationCode::MalformedMetadata),
        (br#"{"uuid":"00112233-4455-4677-8899-aabbccddeeff"} {}"#, ValidationCode::MalformedMetadata),
        (br#"{"uuid":"not-a-uuid"}"#, ValidationCode::InvalidUuid),
        (br#"{"uuid":"00112233-4455-4677-8899-aabbccddeeff","stageNodes":[]}"#, ValidationCode::MissingAsset),
        (br#"{"uuid":"00112233-4455-4677-8899-aabbccddeeff","stageNodes":[{"image":"assets/missing"}]}"#, ValidationCode::MissingAsset),
        (br#"{"uuid":"00112233-4455-4677-8899-aabbccddeeff","stageNodes":[{"image":"../outside"}]}"#, ValidationCode::UnsafePath),
        (br#"{"uuid":"00112233-4455-4677-8899-aabbccddeeff","stageNodes":[{"image":"assets/./cover"}]}"#, ValidationCode::MissingAsset),
    ] {
        let bytes = zip_entries(&[("story.json", story), ("assets/cover", b"asset"), ("thumbnail.png", &thumbnail)])?;
        let mut archive = open(&bytes, b"studio.zip")?;
        let result = archive.inspect_studio_metadata();
        assert!(matches!(result, Err(ArchiveError::Validation(ValidationError { code: actual, .. })) if actual == code), "{result:?}");
    }
    Ok(())
}
#[test]
fn actual_studio_requires_readable_png_thumbnail_after_reference_validation() -> TestResult {
    let story = br#"{"uuid":"00112233-4455-4677-8899-aabbccddeeff","title":"  Story  ","stageNodes":[{"audio":"assets/track"}]}"#;
    for thumbnail in [None, Some(b"not a png".as_slice())] {
        let mut entries = vec![
            ("story.json", story.as_slice()),
            ("assets/track", b"opaque audio".as_slice()),
        ];
        if let Some(bytes) = thumbnail {
            entries.push(("thumbnail.png", bytes));
        }
        let bytes = zip_entries(&entries)?;
        let mut archive = open(&bytes, b"studio.zip")?;
        let code = if thumbnail.is_some() {
            ValidationCode::ArtworkLimit
        } else {
            ValidationCode::MissingEntry
        };
        assert!(
            matches!(archive.inspect_studio_metadata(), Err(ArchiveError::Validation(ValidationError { code: actual, .. })) if actual == code)
        );
    }
    Ok(())
}

fn plain_extra(
    extra: &[(&str, &[u8])],
) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("uuid.bin", &[1; 16]),
        ("ni", b"n"),
        ("li.plain", b"l"),
        ("ri.plain", b"r"),
        ("si.plain", b"s"),
        ("rf/a", b"r"),
        ("sf/a", b"s"),
    ];
    entries.extend_from_slice(extra);
    zip_entries(&entries)
}
#[test]
fn actual_optional_metadata_uuid_text_absence_and_artwork_remain_independent() -> TestResult {
    let cover = png()?;
    let bytes = plain_extra(&[("_metadata.json", br#"{"uuid":"01010101-0101-0101-0101-010101010101","title":"  Embedded  ","description":"  Authored  "}"#), ("_thumbnail.png", &cover)])?;
    let mut archive = open(&bytes, b"story.pk")?;
    let result = archive.inspect_metadata()?;
    assert_eq!(result.text.title, "Embedded");
    assert_eq!(result.text.description, "Authored");
    assert_eq!(result.artwork.ok_or("missing cover")?.bytes, cover);
    for payload in [None, Some(b"null".as_slice()), Some(b"{}".as_slice())] {
        let extra: Vec<_> = payload.map(|p| ("_metadata.json", p)).into_iter().collect();
        let bytes = plain_extra(&extra)?;
        let result = open(&bytes, b"story.pk")?.inspect_metadata()?;
        assert_eq!(result.text, MetadataText::default());
        assert!(result.artwork.is_none());
    }
    Ok(())
}
#[test]
fn actual_optional_metadata_does_not_replace_identity_or_ignore_invalid_selected_bytes()
-> TestResult {
    for (payload, code) in [
        (
            br#"{"uuid":"00112233-4455-4677-8899-aabbccddeeff"}"#.as_slice(),
            ValidationCode::InvalidUuid,
        ),
        (b"", ValidationCode::MalformedMetadata),
        (b"{} {}", ValidationCode::MalformedMetadata),
        (br#"{"title":42}"#, ValidationCode::MalformedMetadata),
    ] {
        let bytes = plain_extra(&[("_metadata.json", payload)])?;
        let result = open(&bytes, b"story.pk")?.inspect_metadata();
        assert!(
            matches!(result, Err(ArchiveError::Validation(ValidationError { code: actual, .. })) if actual == code),
            "{result:?}"
        );
    }
    Ok(())
}
#[test]
fn actual_image_hardening_refuses_retained_previously_accepted_dimensions_and_pixels() -> TestResult
{
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/import-hardening");
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join("images.json"))?)?;
    let rows = manifest["fixtures"]
        .as_array()
        .ok_or("missing image fixture records")?;
    for row in rows {
        let name = row["file"].as_str().ok_or("missing fixture name")?;
        let bytes = fs::read(root.join(name))?;
        assert_eq!(
            librairii_app::metadata::digest(&bytes).to_string(),
            row["sha256"].as_str().ok_or("missing fixture hash")?
        );
        let result = open(&bytes, name.as_bytes())?.inspect_metadata();
        assert!(
            matches!(
                result,
                Err(ArchiveError::Validation(ValidationError {
                    code: ValidationCode::ArtworkLimit,
                    ..
                }))
            ),
            "{name}: {result:?}"
        );
    }
    Ok(())
}
#[test]
fn actual_metadata_and_rejections_match_entire_preserved_inspection_corpus() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus");
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(root.join("inspection.json"))?)?;
    for row in rows {
        let relative = row["file"].as_str().ok_or("missing corpus path")?;
        let name = relative
            .strip_prefix("archives/")
            .ok_or("unexpected fixture path")?;
        let bytes = fs::read(root.join(relative))?;
        let result = open(&bytes, name.as_bytes()).and_then(|mut a| a.inspect_metadata());
        if row["error"].as_str().is_some_and(|s| !s.is_empty()) {
            assert!(
                matches!(result, Err(ArchiveError::Validation(ValidationError { code, .. })) if Some(code.as_str()) == row["error"].as_str()),
                "{name}: {result:?}"
            );
        } else {
            let actual = result?;
            assert_eq!(
                Some(actual.identity.canonical().as_str()),
                row["uuid"].as_str(),
                "{name}"
            );
            assert_eq!(
                Some(actual.format.as_str()),
                row["format"].as_str(),
                "{name}"
            );
            assert_eq!(
                Some(actual.text.title.as_str()),
                row["title"].as_str(),
                "{name}"
            );
            assert_eq!(
                Some(actual.text.description.as_str()),
                row["description"].as_str(),
                "{name}"
            );
            if let Some(art) = actual.artwork {
                assert_eq!(
                    Some(art.entry_name),
                    row["artwork"]["entry"].as_str(),
                    "{name}"
                );
                assert_eq!(
                    Some(
                        librairii_app::metadata::digest(&art.bytes)
                            .to_string()
                            .as_str()
                    ),
                    row["artwork"]["sha256"].as_str(),
                    "{name}"
                );
            } else {
                assert!(row["artwork"].is_null(), "{name}");
            }
        }
    }
    Ok(())
}

fn parity_fixture(name: &str) -> io::Result<Vec<u8>> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/archive-parity")
            .join(name),
    )
}
#[test]
fn independent_seven_codec_streams_preserve_studio_metadata_and_artwork() -> TestResult {
    let original = fixture("clockwork-forest-studio.zip")?;
    let expected_artwork = open(&original, b"story.zip")?
        .inspect_metadata()?
        .artwork
        .ok_or("thumbnail")?
        .bytes;
    for name in [
        "copy",
        "lzma",
        "lzma2",
        "ppmd",
        "bzip2",
        "deflate",
        "bcj",
        "bcj2",
        "ppc",
        "arm",
        "arm64",
        "sparc",
        "delta",
        "lz4",
        "brotli",
        "brotli-envelope",
        "zstd",
        "solid-copy",
        "non-solid-unused-corrupt",
    ] {
        let bytes = parity_fixture(&format!("{name}.7z"))?;
        let mut archive = open(&bytes, b"story.7z")?;
        let metadata = archive
            .inspect_metadata()
            .map_err(|e| format!("{name}: {e}"))?;
        assert_eq!(
            metadata.identity.canonical(),
            "00112233-4455-4677-8899-aabbccddeeff",
            "{name}"
        );
        assert_eq!(metadata.text, MetadataText::default(), "{name}");
        assert_eq!(
            metadata.artwork.ok_or("thumbnail")?.bytes,
            expected_artwork,
            "{name}"
        );
        archive.verify_payloads()?;
    }
    Ok(())
}
#[test]
fn unsupported_go_filters_remain_rejected_when_selected_for_metadata() -> TestResult {
    for name in ["ia64", "thumb", "riscv"] {
        let bytes = parity_fixture(&format!("unsupported-{name}.7z"))?;
        let mut archive = open(&bytes, b"story.7z")?;
        assert!(archive.inspect_metadata().is_err(), "{name}");
    }
    Ok(())
}
#[test]
fn retained_constructor_declarations_and_hardening_fixtures_refuse_before_unbounded_work()
-> TestResult {
    for name in [
        "files-u64max.7z",
        "coders-u64max.7z",
        "header-16m.7z",
        "decoded-header-16m.7z",
        "coders-nine.7z",
        "streams-nine.7z",
        "total-coders-10002.7z",
        "unknown-unused-coder.7z",
        "dictionary-512m.7z",
    ] {
        let bytes = parity_fixture(name)?;
        assert!(open(&bytes, b"story.7z").is_err(), "{name}");
    }
    for name in ["selected-crc.7z", "preceding-crc.7z"] {
        let bytes = parity_fixture(name)?;
        assert!(
            open(&bytes, b"story.7z")?.inspect_metadata().is_err(),
            "{name}"
        );
    }
    let bytes = parity_fixture("path-512.7z")?;
    let one_entry = Limits {
        max_entries: 1,
        ..Limits::default()
    };
    assert!(
        Archive::open(
            Cursor::new(&bytes),
            b"story.7z",
            one_entry,
            &Cancellation::default()
        )
        .is_ok()
    );
    let bytes = parity_fixture("png-header-only.zip")?;
    assert!(open(&bytes, b"story.zip")?.inspect_metadata().is_err());
    Ok(())
}

#[test]
fn zip_checks_unused_payload_crc_while_seven_zip_reads_only_selected_blocks() -> TestResult {
    let zip = parity_fixture("zip-unused-corrupt.zip")?;
    let mut archive = open(&zip, b"story.zip")?;
    assert!(archive.inspect_metadata().is_ok());
    assert!(archive.verify_payloads().is_err());
    let seven = parity_fixture("non-solid-unused-corrupt.7z")?;
    let mut archive = open(&seven, b"story.7z")?;
    assert!(archive.inspect_metadata().is_ok());
    assert!(archive.verify_payloads().is_ok());
    Ok(())
}

#[test]
fn retained_codec_and_hardening_manifest_hashes_and_expected_outcomes_are_enforced() -> TestResult {
    use librairii_app::importing::Inspector;
    let manifest: serde_json::Value = serde_json::from_slice(&parity_fixture("manifest.json")?)?;
    let mut failures = Vec::new();
    for row in manifest["fixtures"].as_array().ok_or("fixture array")? {
        let name = row["file"].as_str().ok_or("fixture name")?;
        let bytes = parity_fixture(name)?;
        assert_eq!(
            librairii_app::metadata::digest(&bytes).to_string(),
            row["sha256"].as_str().ok_or("sha")?,
            "{name}"
        );
        if name == "path-512.7z" {
            continue;
        }
        let suffix = if name.ends_with(".zip") {
            b"story.zip".as_slice()
        } else {
            b"story.7z".as_slice()
        };
        let outcome =
            ArchiveInspector.inspect(&mut Cursor::new(&bytes), suffix, &Cancellation::default());
        let actual = match outcome {
            Ok(_) => "accepted",
            Err(librairii_app::importing::InspectionError::Validation(ref e)) => e.code.as_str(),
            Err(librairii_app::importing::InspectionError::Cancelled) => "cancelled",
        };
        let expected = row["rust"].as_str().ok_or("expected outcome")?;
        if actual != expected {
            failures.push(format!("{name}: {actual}; required {expected}"));
        }
    }
    assert!(
        failures.is_empty(),
        "Unresolved import parity: {failures:?}"
    );
    Ok(())
}

#[test]
fn constructor_budgets_can_only_reduce_production_allocation_ceilings() -> TestResult {
    let bytes = fixture("studio.7z")?;
    for decoder in [
        DecoderLimits {
            header_bytes: 0,
            memory_bytes: DECODER_BYTES,
        },
        DecoderLimits {
            header_bytes: HEADER_BYTES + 1,
            memory_bytes: DECODER_BYTES,
        },
        DecoderLimits {
            header_bytes: HEADER_BYTES,
            memory_bytes: 0,
        },
        DecoderLimits {
            header_bytes: HEADER_BYTES,
            memory_bytes: DECODER_BYTES + 1,
        },
    ] {
        assert!(matches!(
            Archive::open_bounded(
                Cursor::new(&bytes),
                b"story.7z",
                Limits::default(),
                decoder,
                &Cancellation::default()
            ),
            Err(ArchiveError::Validation(ValidationError {
                code: ValidationCode::InvalidLimits,
                ..
            }))
        ));
    }
    Ok(())
}

#[test]
fn unqualified_brotli_payload_decoder_remains_disabled() -> TestResult {
    let bytes = parity_fixture("large-window-brotli.7z")?;
    let mut archive = open(&bytes, b"story.7z")?;
    assert!(archive.inspect_metadata().is_err());
    // The unqualified large-window extension stays disabled; ordinary Brotli
    // is accepted by the separately enforced Go parity manifest.
    Ok(())
}

#[test]
fn patched_header_graph_and_brotli_support_fragmented_source_reads_and_strict_windows() -> TestResult
{
    struct Fragmented(Cursor<Vec<u8>>);
    impl Read for Fragmented {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let n = out.len().min(1);
            self.0.read(&mut out[..n])
        }
    }
    impl Seek for Fragmented {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            self.0.seek(p)
        }
    }
    for name in [
        "brotli.7z",
        "brotli-envelope.7z",
        "encoded-header-brotli.7z",
        "encoded-header-bcj2.7z",
    ] {
        let bytes = parity_fixture(name)?;
        let mut archive = Archive::open(
            Fragmented(Cursor::new(bytes)),
            b"story.7z",
            Limits::default(),
            &Cancellation::default(),
        )?;
        assert_eq!(
            archive.inspect_metadata()?.identity.canonical(),
            "00112233-4455-4677-8899-aabbccddeeff"
        );
    }
    use librairii_app::importing::Inspector;
    for name in ["large-window-brotli.7z", "large-window-encoded-brotli.7z"] {
        let bytes = parity_fixture(name)?;
        let result = ArchiveInspector.inspect(
            &mut Cursor::new(bytes),
            b"story.7z",
            &Cancellation::default(),
        );
        assert!(result.is_err(), "{name}");
    }
    Ok(())
}

#[test]
fn encoded_header_patch_rejects_graph_mismatch_and_aggregate_memory_before_decode() -> TestResult {
    use sevenz_turbo::{ArchiveLimits, ArchiveReader, Error, Password};
    for name in [
        "encoded-header-pack-mismatch.7z",
        "encoded-header-memory-chain.7z",
    ] {
        let bytes = parity_fixture(name)?;
        let limits = ArchiveLimits {
            max_end_header_bytes: HEADER_BYTES,
            max_header_unpacked_bytes: HEADER_BYTES,
            memory_limit_bytes: DECODER_BYTES,
            ..ArchiveLimits::default()
        };
        let result = std::panic::catch_unwind(|| {
            ArchiveReader::with_limits(Cursor::new(&bytes), Password::empty(), limits)
        });
        let refused = result
            .map_err(|_| "encoded header panicked")?
            .err()
            .ok_or("header admitted")?;
        if name == "encoded-header-memory-chain.7z" {
            assert!(
                matches!(refused, Error::MemoryLimited { .. }),
                "{refused:?}"
            );
        }
        assert!(matches!(
            open(&bytes, b"story.7z"),
            Err(ArchiveError::Validation(_))
        ));
    }
    Ok(())
}
