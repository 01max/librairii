use super::*;
use crate::import::{
    CompressionAccounting, EntryInfo, InventoryBuilder, UuidSource, project_studio_json,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const UUID: &str = "00112233-4455-4677-8899-aabbccddeeff";

fn inventory(entries: &[(&str, bool)]) -> Result<Inventory, ValidationError> {
    let mut builder = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    for &(name, is_directory) in entries {
        builder.push(EntryInfo {
            name: name.as_bytes(),
            compressed_bytes: 1,
            uncompressed_bytes: 1,
            is_directory,
            is_link: false,
        })?;
    }
    builder.finish()
}

fn selected() -> Result<Inventory, ValidationError> {
    inventory(&[(ENTRY, false)])
}

fn primary() -> Result<ImportIdentity, ValidationError> {
    ImportIdentity::parse_text(UUID.as_bytes())
}

fn project(payload: &[u8]) -> Result<Option<MetadataText>, ValidationError> {
    project_optional_metadata(&selected()?, Some(payload), &primary()?, Limits::default())
}

#[test]
fn optional_metadata_selection_distinguishes_absence_directory_unselected_and_empty() -> TestResult
{
    for entries in [vec![], vec![(ENTRY, true)], vec![("_Metadata.json", false)]] {
        let files = inventory(&entries)?;
        for payload in [None, Some(b"".as_slice()), Some(b"not JSON")] {
            assert_eq!(
                project_optional_metadata(&files, payload, &primary()?, Limits::default())?,
                None
            );
        }
    }
    assert_eq!(
        project_optional_metadata(&selected()?, None, &primary()?, Limits::default())?,
        None
    );
    let error = project(b"").err().ok_or("empty should fail")?;
    assert_eq!(
        (error.code, error.entry),
        (
            ValidationCode::MalformedMetadata,
            Some(ENTRY.as_bytes().to_vec())
        )
    );
    for payload in [
        b"null".as_slice(),
        b"{}",
        br#"{"uuid":null,"title":null,"description":""}"#,
    ] {
        assert_eq!(project(payload)?, Some(MetadataText::default()));
    }
    Ok(())
}

#[test]
fn optional_metadata_three_field_shape_skips_studio_and_unknown_types() -> TestResult {
    for value in [
        "false",
        "null",
        "17",
        "1e999999999999999999999",
        "[]",
        r#"{"image":false,"audio":{}}"#,
    ] {
        let bytes =
            format!(r#"{{"stageNodes":{value},"image":{value},"audio":{value},"title":" yes "}}"#);
        assert_eq!(project(bytes.as_bytes())?.ok_or("selected")?.title, "yes");
    }
    // These are typed failures for Studio, but not optional metadata fields.
    let bytes = br#"{"stageNodes":false,"image":[],"audio":{},"unknown":{"uuid":false}}"#;
    assert_eq!(project(bytes)?, Some(MetadataText::default()));
    assert_eq!(
        project_studio_json(&selected()?, bytes, Limits::default()).map_err(|e| e.code),
        Err(ValidationCode::MalformedMetadata)
    );
    Ok(())
}

#[test]
fn optional_metadata_empty_uuid_is_allowed_without_changing_primary_identity() -> TestResult {
    let raw = "URN:UUID:00112233-4455-4677-8899-AABBCCDDEEFF";
    let identity = ImportIdentity::parse_text(raw.as_bytes())?;
    let original = identity.clone();
    for bytes in [
        b"null".as_slice(),
        b"{}",
        br#"{"uuid":""}"#,
        br#"{"uuid":null}"#,
        br#"{"uuid":"wrong","uuid":"","uuid":null}"#,
    ] {
        assert_eq!(
            project_optional_metadata(&selected()?, Some(bytes), &identity, Limits::default())?,
            Some(MetadataText::default())
        );
        assert_eq!(identity, original);
        assert_eq!(identity.raw(), &UuidSource::Text(raw.as_bytes().to_vec()));
        assert_eq!(
            project_studio_json(&selected()?, bytes, Limits::default()).map_err(|e| e.code),
            Err(ValidationCode::InvalidUuid)
        );
    }
    Ok(())
}

#[test]
fn optional_metadata_uuid_uses_untrimmed_legacy_forms_and_canonical_comparison() -> TestResult {
    let identity = ImportIdentity::from_plain_bytes(&[
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x46, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ])?;
    let original = identity.clone();
    for raw in [
        UUID.to_owned(),
        "00112233445546778899AABBCCDDEEFF".to_owned(),
        format!("URN:UUID:{UUID}"),
        format!("{{{UUID}}}"),
        // Frozen 38-byte Parse inspects only its middle36 bytes: even these
        // delimiter bytes are accepted WITHOUT any whitespace trimming.
        format!(" {UUID} "),
        format!("x{UUID}y"),
    ] {
        let bytes = format!(r#"{{"uuid":"{raw}","title":" kept "}}"#);
        let text = project_optional_metadata(
            &selected()?,
            Some(bytes.as_bytes()),
            &identity,
            Limits::default(),
        )?
        .ok_or("selected")?;
        assert_eq!(text.title, "kept");
        assert_eq!(identity, original);
    }
    for raw in [
        format!(" {UUID}"),
        format!("{UUID} "),
        format!("  {UUID}  "),
        format!("\u{00a0}{UUID}\u{00a0}"),
        " ".to_owned(),
        "not UUID".to_owned(),
        "00000000-0000-0000-0000-000000000000".to_owned(),
        "00112233-4455-4677-8899-aabbccddee00".to_owned(),
    ] {
        let bytes = format!(r#"{{"uuid":"{raw}"}}"#);
        let error = project(bytes.as_bytes()).err().ok_or("UUID should fail")?;
        assert_eq!(
            (error.code, error.entry),
            (ValidationCode::InvalidUuid, Some(ENTRY.as_bytes().to_vec()))
        );
    }
    Ok(())
}

#[test]
fn optional_metadata_duplicates_null_folding_and_first_type_error_are_ordered() -> TestResult {
    let bytes = br#"{"uuid":"bad","UUID":"","uuid":null,"title":" first ","TITLE":null,"title":" last ","description":" kept ","de\u017fcription":null}"#;
    assert_eq!(
        project(bytes)?,
        Some(MetadataText {
            title: "last".into(),
            description: "kept".into()
        })
    );
    assert_eq!(
        project(br#"{"uuid":"bad","uuid":null}"#).map_err(|e| e.code),
        Err(ValidationCode::InvalidUuid)
    );
    // Dotted/dotless I do not fold into the ASCII schema.
    assert_eq!(
        project(br#"{"t\u0130tle":false,"t\u0131tle":17,"descriptionX":{}}"#)?,
        Some(MetadataText::default())
    );
    for (bytes, field) in [
        (
            br#"{"title":17,"description":false,"title":"later"} broken"#.as_slice(),
            "title",
        ),
        (
            br#"{"description":{},"uuid":false,"description":null} null"#,
            "description",
        ),
        (br#"{"UUID":false,"uuid":""} ["#, "uuid"),
    ] {
        assert!(matches!(decode(bytes), Err(DecodeFailure::Type {field: actual}) if actual==field));
        assert_eq!(
            project(bytes).map_err(|e| e.code),
            Err(ValidationCode::MalformedMetadata)
        );
    }
    Ok(())
}

#[test]
fn optional_metadata_first_syntax_then_types_then_eof_then_uuid_priority() {
    for bytes in [
        br#"{"title":false,"stageNodes":[0,]}"#.as_slice(),
        br#"{"uuid":false,"x":"\v"}"#,
    ] {
        assert!(matches!(decode(bytes), Err(DecodeFailure::Syntax { .. })));
    }
    for bytes in [b"01".as_slice(), b"trueX", br#""text" broken"#] {
        assert!(matches!(
            decode(bytes),
            Err(DecodeFailure::Type { field: "metadata" })
        ));
    }
    for bytes in [
        br#"{"uuid":"wrong"} {}"#.as_slice(),
        b"null garbage",
        b"{}\xc2\xa0",
    ] {
        assert!(matches!(decode(bytes), Err(DecodeFailure::Trailing { .. })));
        assert_eq!(
            project(bytes).map_err(|e| e.code),
            Err(ValidationCode::MalformedMetadata)
        );
    }
}

#[test]
fn optional_metadata_preserves_replacement_origins_and_boundary_only_text() -> TestResult {
    let bytes = b"{\"title\":\" \\u000b\xed\xa0\x80 \\u000c\",\"description\":\"\\u00a0\\uD800 a  b\\n c \\u3000\"}";
    assert_eq!(
        project(bytes)?,
        Some(MetadataText {
            title: "\u{fffd}\u{fffd}\u{fffd}".into(),
            description: "\u{fffd} a  b\n c".into()
        })
    );
    let bytes = br#"{"title":" \u180e \u00e9 e\u0301 \u200b ","description":"\uFEFF"}"#;
    assert_eq!(
        project(bytes)?,
        Some(MetadataText {
            title: "\u{180e} é e\u{0301} \u{200b}".into(),
            description: "\u{feff}".into()
        })
    );
    for bytes in [b"\x0b{}".as_slice(), b"{}\x0c"] {
        assert_eq!(
            project(bytes).map_err(|e| e.code),
            Err(ValidationCode::MalformedMetadata)
        );
    }
    Ok(())
}

#[test]
fn optional_metadata_supplied_original_byte_cap_and_depth_are_separate_from_heap_reads()
-> TestResult {
    for length in [(1 << 20) - 1, 1 << 20, (1 << 20) + 1] {
        let mut bytes = b"{}".to_vec();
        bytes.resize(length, b' ');
        let result = project(&bytes);
        assert_eq!(result.is_ok(), length <= 1 << 20);
        if length > 1 << 20 {
            assert_eq!(
                result.map_err(|e| e.code),
                Err(ValidationCode::MetadataLimit)
            );
        }
    }
    let mut bytes = b"{\"title\":\"".to_vec();
    bytes.extend([0xff; 30]);
    bytes.extend_from_slice(b"\"}");
    let limits = Limits {
        max_metadata_bytes: i64::try_from(bytes.len())?,
        ..Limits::default()
    };
    let projected = project_optional_metadata(&selected()?, Some(&bytes), &primary()?, limits)?
        .ok_or("selected")?;
    assert_eq!(projected.title.len(), 90);
    let smaller = Limits {
        max_metadata_bytes: limits.max_metadata_bytes - 1,
        ..limits
    };
    assert_eq!(
        project_optional_metadata(&selected()?, Some(&bytes), &primary()?, smaller)
            .map_err(|e| e.code),
        Err(ValidationCode::MetadataLimit)
    );
    for (nested, accepted) in [(9_999, true), (10_000, false)] {
        let bytes = format!(
            r#"{{"stageNodes":{}0{}}}"#,
            "[".repeat(nested),
            "]".repeat(nested)
        );
        assert_eq!(project(bytes.as_bytes()).is_ok(), accepted);
    }
    let invalid = Limits {
        max_metadata_bytes: 0,
        ..Limits::default()
    };
    assert_eq!(
        project_optional_metadata(&inventory(&[])?, None, &primary()?, invalid).map_err(|e| e.code),
        Err(ValidationCode::InvalidLimits)
    );
    Ok(())
}
