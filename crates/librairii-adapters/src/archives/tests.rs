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
