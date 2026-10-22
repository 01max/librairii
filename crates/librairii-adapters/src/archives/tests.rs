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
