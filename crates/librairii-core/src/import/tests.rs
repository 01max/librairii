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
