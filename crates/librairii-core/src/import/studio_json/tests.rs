use super::*;
use crate::import::{CompressionAccounting, EntryInfo, InventoryBuilder, UuidSource};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const UUID: &str = "00112233-4455-4677-8899-aabbccddeeff";

fn inventory(names: &[&str]) -> Result<Inventory, ValidationError> {
    let mut builder = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    for name in names {
        builder.push(EntryInfo {
            name: name.as_bytes(),
            compressed_bytes: 1,
            uncompressed_bytes: 1,
            is_directory: false,
            is_link: false,
        })?;
    }
    builder.finish()
}

fn story(nodes: &str, extra: &str) -> Vec<u8> {
    format!(r#"{{"uuid":"{UUID}","stageNodes":{nodes}{extra}}}"#).into_bytes()
}

fn valid() -> Vec<u8> {
    story(r#"[{"image":"assets/a"}]"#, "")
}

fn supplied_files() -> Result<Inventory, ValidationError> {
    inventory(&[
        "story.json",
        "thumbnail.png",
        "assets/a",
        "assets/b",
        "assets/c",
    ])
}

#[test]
fn studio_json_supplied_bytes_decode_then_existing_policy_preserve_identity() -> TestResult {
    let files = supplied_files()?;
    for raw in [
        UUID,
        "00112233445546778899AABBCCDDEEFF",
        "URN:UUID:00112233-4455-4677-8899-AABBCCDDEEFF",
    ] {
        let bytes = format!(
            r#" {{"uuid":"{raw}","stageNodes":[{{"image":"assets\/a"}}],"title":" x ","description":null}} "#
        );
        let identity = validate_studio_json(&files, bytes.as_bytes(), Limits::default())?;
        assert_eq!(identity.raw(), &UuidSource::Text(raw.as_bytes().to_vec()));
        assert_eq!(identity.canonical(), UUID);
    }
    Ok(())
}

#[test]
fn studio_json_first_value_syntax_outranks_types_but_types_precede_trailing() {
    for input in [
        br#"{"uuid":false,"unknown":[0,]}"#.as_slice(),
        br#"{"title":true,"unknown":"\v"}"#,
        br#"{"description":{},"x":01}"#,
    ] {
        assert!(matches!(decode(input), Err(DecodeFailure::Syntax { .. })));
    }
    assert_eq!(
        decode(br#"{"uuid":false} broken"#).map(|_| ()),
        Err(DecodeFailure::Type { field: "uuid" })
    );
    assert_eq!(
        decode(br#"{"title":true,"uuid":false} ["#).map(|_| ()),
        Err(DecodeFailure::Type { field: "title" })
    );
    assert_eq!(
        decode(br#"{"description":[],"title":true} null"#).map(|_| ()),
        Err(DecodeFailure::Type {
            field: "description"
        })
    );
    for input in [b"01".as_slice(), b"trueX", br#""text"garbage"#] {
        assert_eq!(
            decode(input).map(|_| ()),
            Err(DecodeFailure::Type { field: "story" })
        );
    }
}

#[test]
fn studio_json_successful_decode_checks_eof_before_uuid_or_assets() -> TestResult {
    let files = inventory(&[])?;
    for input in [
        br#"{"uuid":"bad"} {}"#.as_slice(),
        b"null null",
        b"{}garbage",
        b"{}\xc2\xa0",
    ] {
        assert!(matches!(decode(input), Err(DecodeFailure::Trailing { .. })));
        assert_eq!(
            validate_studio_json(&files, input, Limits::default()).map_err(|e| e.code),
            Err(ValidationCode::MalformedMetadata)
        );
    }
    assert_eq!(
        validate_studio_json(&files, b"null", Limits::default()).map_err(|e| e.code),
        Err(ValidationCode::InvalidUuid)
    );
    assert!(decode(b"{} \t\r\n").is_ok());
    Ok(())
}

#[test]
fn studio_json_known_string_null_duplicates_and_saved_first_type_error() -> TestResult {
    let bytes = story(
        "[]",
        r#", "uuid":null,"title":"first","TITLE":null,"title":"last","description":"kept","DESCRIPTION":null"#,
    );
    let decoded = decode(&bytes)?;
    assert_eq!(
        (
            decoded.uuid.as_str(),
            decoded.title.as_str(),
            decoded.description.as_str()
        ),
        (UUID, "last", "kept")
    );
    for (input, field) in [
        (
            story("[]", r#", "title":false,"title":"repaired""#),
            "title",
        ),
        (
            story("[]", r#", "description":{},"description":null"#),
            "description",
        ),
        (story("[]", r#", "uuid":17,"uuid":"valid later""#), "uuid"),
        (
            story("[]", r#", "title":[],"description":false,"title":"ok""#),
            "title",
        ),
    ] {
        assert_eq!(
            decode(&input).map(|_| ()),
            Err(DecodeFailure::Type { field })
        );
    }
    Ok(())
}

#[test]
fn studio_json_unknown_subtrees_skip_huge_numbers_without_float_or_new_strictness() -> TestResult {
    let bytes = story(
        r#"[{"image":"assets/a","unknown":{"image":false}}]"#,
        r#", "unknown":{"large":1e999999999999999999999,"negative":-999999999999999999999999999999999999999999999999,"null":null,"array":[true,{},"\uD800"]},"stageNodesX":false"#,
    );
    validate_studio_json(&supplied_files()?, &bytes, Limits::default())?;
    Ok(())
}

#[test]
fn studio_json_duplicate_arrays_reuse_hidden_backing_cells_after_shrink_and_growth() -> TestResult {
    let bytes = story(
        r#"[{"image":"assets/a"},{"audio":"assets/b"}]"#,
        r#", "stageNodes":[{"audio":"assets/c"}],"stageNodes":[{},null]"#,
    );
    let decoded = decode(&bytes)?;
    assert_eq!(decoded.length, 2);
    assert_eq!(
        (
            decoded.cells[0].image.as_str(),
            decoded.cells[0].audio.as_str()
        ),
        ("assets/a", "assets/c")
    );
    assert_eq!(
        (
            decoded.cells[1].image.as_str(),
            decoded.cells[1].audio.as_str()
        ),
        ("", "assets/b")
    );
    validate_studio_json(&supplied_files()?, &bytes, Limits::default())?;
    Ok(())
}

#[test]
fn studio_json_null_and_empty_array_reset_cells_but_null_nodes_and_fields_do_not() -> TestResult {
    for reset in ["null", "[]"] {
        let bytes = story(
            r#"[{"image":"assets/a"},{"audio":"assets/b"}]"#,
            &format!(r#", "stageNodes":{reset},"stageNodes":[{{}},null]"#),
        );
        let decoded = decode(&bytes)?;
        assert!(
            decoded
                .cells
                .iter()
                .all(|node| node.image.is_empty() && node.audio.is_empty())
        );
        assert_eq!(
            validate_studio_json(&supplied_files()?, &bytes, Limits::default()).map_err(|e| e.code),
            Err(ValidationCode::MissingAsset)
        );
    }
    let bytes = story(
        r#"[{"image":"assets/a","audio":"assets/b"}]"#,
        r#", "stageNodes":[{"image":null,"audio":null}]"#,
    );
    let decoded = decode(&bytes)?;
    assert_eq!(
        (
            decoded.cells[0].image.as_str(),
            decoded.cells[0].audio.as_str()
        ),
        ("assets/a", "assets/b")
    );
    let null_node = story(r#"[{"image":"assets/a"}]"#, r#", "stageNodes":[null]"#);
    validate_studio_json(&supplied_files()?, &null_node, Limits::default())?;
    Ok(())
}

#[test]
fn studio_json_image_audio_and_node_type_errors_keep_input_order() {
    for (nodes, field) in [
        (
            r#"[{"audio":false,"image":17},{"audio":"ok"}]"#,
            "stageNodes.audio",
        ),
        (r#"[{"image":false,"image":"ok"}]"#, "stageNodes.image"),
        (r#"[{},[],{"image":false}]"#, "stageNodes"),
        (r#"[true]"#, "stageNodes"),
        (r#"{"image":"assets/a"}"#, "stageNodes"),
    ] {
        assert_eq!(
            decode(&story(nodes, "")).map(|_| ()),
            Err(DecodeFailure::Type { field })
        );
    }
}

#[test]
fn studio_json_schema_casefold_uses_long_s_but_not_dotted_or_dotless_i() -> TestResult {
    let input = format!(
        r#"{{"UUID":"{UUID}","deſcription":"known","ſtageNodeſ":[{{"IMAGE":"assets/a"}}]}}"#
    );
    let decoded = decode(input.as_bytes())?;
    assert_eq!(decoded.description, "known");
    validate_studio_json(&supplied_files()?, input.as_bytes(), Limits::default())?;
    for key in ["uuİd", "uuıd", "uu\u{212a}d"] {
        let input = format!(r#"{{"{key}":"{UUID}","stageNodes":[{{"image":"assets/a"}}]}}"#);
        assert_eq!(
            validate_studio_json(&supplied_files()?, input.as_bytes(), Limits::default())
                .map_err(|e| e.code),
            Err(ValidationCode::InvalidUuid)
        );
    }
    let input = story(r#"[{"İmage":"assets/a","ımage":"assets/a"}]"#, "");
    assert_eq!(
        validate_studio_json(&supplied_files()?, &input, Limits::default()).map_err(|e| e.code),
        Err(ValidationCode::MissingAsset)
    );
    let input =
        format!(r#"{{"u\u0075id":"{UUID}","sta\u0067eNodes":[{{"\u0069mage":"assets/a"}}]}}"#);
    validate_studio_json(&supplied_files()?, input.as_bytes(), Limits::default())?;
    Ok(())
}

#[test]
fn studio_json_raw_invalid_utf8_and_escaped_surrogates_have_distinct_replacements() -> TestResult {
    for (token, expected) in [
        (b"\"\xed\xa0\x80\"".as_slice(), "\u{fffd}\u{fffd}\u{fffd}"),
        (br#""\uD800""#, "\u{fffd}"),
        (b"\"\xe2\x82\"", "\u{fffd}\u{fffd}"),
        (b"\"\xc0\x80\"", "\u{fffd}\u{fffd}"),
        (
            br#""\uDC00\uD800\uD800\uDC00""#,
            "\u{fffd}\u{fffd}\u{10000}",
        ),
        (br#""\uD800\n\u0041""#, "\u{fffd}\nA"),
        (br#""\uDBFF\uDFFF""#, "\u{10ffff}"),
        (b"\"\xf0\x90\x80\x80\xef\xbf\xbd\"", "\u{10000}\u{fffd}"),
    ] {
        scanner::value(token, 0)?;
        assert_eq!(strings::unquote(token), expected);
    }
    let mut input = valid();
    input.pop();
    input.extend_from_slice(b",\"title\":\"\xff\",\"\xff\":true}");
    assert_eq!(decode(&input)?.title, "\u{fffd}");
    validate_studio_json(&supplied_files()?, &input, Limits::default())?;
    Ok(())
}

#[test]
fn studio_json_replacement_and_escape_decoded_references_use_exact_inventory_keys() -> TestResult {
    let files = inventory(&[
        "thumbnail.png",
        "assets/\u{fffd}",
        "assets/\u{fffd}\u{fffd}\u{fffd}",
    ])?;
    let escaped = story(r#"[{"image":"assets/\uD800"}]"#, "");
    validate_studio_json(&files, &escaped, Limits::default())?;
    let mut raw = format!(r#"{{"uuid":"{UUID}","stageNodes":[{{"image":"assets/"#).into_bytes();
    raw.extend_from_slice(b"\xed\xa0\x80\"}]}");
    validate_studio_json(&files, &raw, Limits::default())?;
    let one_file = inventory(&["thumbnail.png", "assets/\u{fffd}"])?;
    let error =
        validate_studio_json(&one_file, &raw, Limits::default()).map_err(|e| (e.code, e.entry));
    assert_eq!(
        error,
        Err((
            ValidationCode::MissingAsset,
            Some("assets/\u{fffd}\u{fffd}\u{fffd}".as_bytes().to_vec())
        ))
    );
    Ok(())
}

#[test]
fn studio_json_grammar_rejects_controls_bad_escapes_and_container_number_errors() {
    for input in [
        b"".as_slice(),
        b" \r\n",
        b"\xef\xbb\xbf{}",
        b"//comment\n{}",
        br#"{"x":"\'"}"#,
        br#"{"x":"\u123"}"#,
        br#"{"x":"\x20"}"#,
        b"{\"x\":\"\x00\"}",
        br#"{"x":01}"#,
        br#"{"x":+1}"#,
        br#"{"x":1.}"#,
        br#"{"x":1e+}"#,
        br#"{"x":NaN}"#,
        br#"{"x":trueX}"#,
        br#"{"x":[0,]}"#,
        br#"{"x":0,}"#,
        br#"{"x" 0}"#,
        br#"{1:0}"#,
        br#"{"x":{]}"#,
        br#"{"x":[,]}"#,
    ] {
        assert!(
            matches!(decode(input), Err(DecodeFailure::Syntax { .. })),
            "{input:?}"
        );
    }
}

#[test]
fn studio_json_grammar_accepts_unknown_valid_scalars_escapes_and_ascii_whitespace() -> TestResult {
    for unknown in [
        "0",
        "-0",
        "123",
        "-17.25e+003",
        "0E-0",
        "true",
        "false",
        "null",
        r#""\b\f\n\r\t\\\/\"\u0000\uD800""#,
        "{\"x\":[],\"x\":{}}",
        "[{},[],null,0]",
    ] {
        let input = story(
            r#"[{"image":"assets/a"}]"#,
            &format!(",\r\n\t\"unknown\":{unknown}"),
        );
        validate_studio_json(&supplied_files()?, &input, Limits::default())?;
    }
    Ok(())
}

#[test]
fn studio_json_depth_10000_is_iterative_including_ignored_subtrees() -> TestResult {
    let files = supplied_files()?;
    for (nested, accepted) in [(129, true), (9_999, true), (10_000, false)] {
        let extra = format!(",\"unknown\":{}0{}", "[".repeat(nested), "]".repeat(nested));
        let input = story(r#"[{"image":"assets/a"}]"#, &extra);
        let result = validate_studio_json(&files, &input, Limits::default());
        assert_eq!(result.is_ok(), accepted, "depth {}", nested + 1);
        if !accepted {
            assert_eq!(
                result.map_err(|e| e.code),
                Err(ValidationCode::MalformedMetadata)
            );
        }
    }
    let top = format!("{}0{}", "[".repeat(10_000), "]".repeat(10_000));
    assert_eq!(
        decode(top.as_bytes()).map(|_| ()),
        Err(DecodeFailure::Type { field: "story" })
    );
    Ok(())
}

#[test]
fn studio_json_supplied_byte_limits_count_original_bytes_not_replacement_output() -> TestResult {
    let files = supplied_files()?;
    for length in [(1 << 20) - 1, 1 << 20, (1 << 20) + 1] {
        let mut bytes = valid();
        bytes.resize(length, b' ');
        let result = validate_studio_json(&files, &bytes, Limits::default());
        assert_eq!(result.is_ok(), length <= 1 << 20);
        if length > 1 << 20 {
            assert_eq!(
                result.map_err(|e| e.code),
                Err(ValidationCode::MetadataLimit)
            );
        }
    }
    let mut bytes = valid();
    bytes.pop();
    bytes.extend_from_slice(b",\"title\":\"");
    bytes.extend(vec![0xff; 1_000]);
    bytes.extend_from_slice(b"\"}");
    let limits = Limits {
        max_metadata_bytes: i64::try_from(bytes.len())?,
        ..Limits::default()
    };
    assert_eq!(decode(&bytes)?.title.len(), 3_000);
    validate_studio_json(&files, &bytes, limits)?;
    let lower = Limits {
        max_metadata_bytes: limits.max_metadata_bytes - 1,
        ..limits
    };
    assert_eq!(
        validate_studio_json(&files, &bytes, lower).map_err(|e| e.code),
        Err(ValidationCode::MetadataLimit)
    );
    let invalid = Limits {
        max_metadata_bytes: 0,
        ..limits
    };
    assert_eq!(
        validate_studio_json(&files, b"broken", invalid).map_err(|e| e.code),
        Err(ValidationCode::InvalidLimits)
    );
    Ok(())
}

#[test]
fn studio_json_reference_count_is_not_the_archive_entry_limit() -> TestResult {
    let nodes = format!("[{}]", vec![r#"{"image":"assets/a"}"#; 10_001].join(","));
    validate_studio_json(&supplied_files()?, &story(&nodes, ""), Limits::default())?;
    Ok(())
}

#[test]
fn studio_json_decode_uuid_refs_and_thumbnail_preserve_pipeline_error_order() -> TestResult {
    let no_files = inventory(&[])?;
    for (input, code, entry) in [
        (
            story(r#"[{"image":"../bad"}]"#, r#", "title":false"#),
            ValidationCode::MalformedMetadata,
            "story.json",
        ),
        (
            story(r#"[{"image":"../bad"}]"#, r#", "uuid":"bad""#),
            ValidationCode::InvalidUuid,
            "story.json",
        ),
        (
            story(r#"[{"audio":"missing","image":"assets/first"}]"#, ""),
            ValidationCode::MissingAsset,
            "assets/first",
        ),
        (
            story("[null,{}]", ""),
            ValidationCode::MissingAsset,
            "stageNodes",
        ),
    ] {
        assert_eq!(
            validate_studio_json(&no_files, &input, Limits::default())
                .map_err(|e| (e.code, e.entry)),
            Err((code, Some(entry.as_bytes().to_vec())))
        );
    }
    let files = inventory(&["assets/a"])?;
    assert_eq!(
        validate_studio_json(&files, &valid(), Limits::default()).map_err(|e| (e.code, e.entry)),
        Err((
            ValidationCode::MissingEntry,
            Some(b"thumbnail.png".to_vec())
        ))
    );
    let nul = story(r#"[{"image":"assets/\u0000"}]"#, "");
    let error = validate_studio_json(&supplied_files()?, &nul, Limits::default()).err();
    assert!(error.is_some_and(|e| e.code == ValidationCode::UnsafePath && e.cause.is_some()));
    Ok(())
}

#[test]
fn studio_text_projection_preserves_raw_identity_and_only_trims_boundaries() -> TestResult {
    let raw = "URN:UUID:00112233-4455-4677-8899-AABBCCDDEEFF";
    let bytes = format!(
        r#"{{"uuid":"{raw}","stageNodes":[{{"image":"assets/a"}}],"title":" \tÉté  日本語\r\n","description":"\u00a0E\u0301\n x\u3000"}}"#
    );
    let projected = project_studio_json(&supplied_files()?, bytes.as_bytes(), Limits::default())?;
    assert_eq!(
        projected.identity.raw(),
        &UuidSource::Text(raw.as_bytes().to_vec())
    );
    assert_eq!(projected.identity.canonical(), UUID);
    assert_eq!(projected.text.title, "Été  日本語");
    assert_eq!(projected.text.description, "E\u{0301}\n x");
    assert_eq!(
        validate_studio_json(&supplied_files()?, bytes.as_bytes(), Limits::default())?,
        projected.identity
    );
    // A supplied name and presence bit is sufficient for this policy API:
    // no PNG bytes were supplied or verified.
    Ok(())
}

#[test]
fn studio_text_frozen_whitespace_and_nonspace_boundaries() -> TestResult {
    let files = supplied_files()?;
    let whitespace = [
        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x20, 0x85, 0xa0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003,
        0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f,
        0x3000,
    ];
    for cp in whitespace {
        let input = story(
            r#"[{"image":"assets/a"}]"#,
            &format!(
                r#", "title":"\u{cp:04x}left\u{cp:04x}right\u{cp:04x}","description":"\u{cp:04x}\u{cp:04x}""#
            ),
        );
        let text = project_studio_json(&files, &input, Limits::default())?.text;
        assert_eq!(
            text.title,
            format!("left{}right", char::from_u32(cp).ok_or("scalar")?)
        );
        assert_eq!(text.description, "", "U+{cp:04X}");
    }
    for cp in [0, 0x08, 0x0e, 0x180e, 0x200b, 0xfeff] {
        let input = story(
            r#"[{"image":"assets/a"}]"#,
            &format!(r#", "title":" \u{cp:04x} x \u{cp:04x} ","description":"\u{cp:04x}""#),
        );
        let text = project_studio_json(&files, &input, Limits::default())?.text;
        let ch = char::from_u32(cp).ok_or("scalar")?;
        assert_eq!(text.title, format!("{ch} x {ch}"));
        assert_eq!(text.description, ch.to_string());
    }
    // VT/FF trim inside a decoded string but are not JSON whitespace.
    for input in [b"\x0b{}".as_slice(), b"{}\x0c"] {
        assert_eq!(
            project_studio_json(&files, input, Limits::default()).map_err(|e| e.code),
            Err(ValidationCode::MalformedMetadata)
        );
    }
    Ok(())
}

#[test]
fn studio_text_known_duplicates_null_and_fold_keep_decoder_semantics() -> TestResult {
    let bytes = story(
        r#"[{"image":"assets/a"}]"#,
        r#", "title":" old ","TITLE":null,"title":" final ","description":" kept ","deſcription":null"#,
    );
    let projected = project_studio_json(&supplied_files()?, &bytes, Limits::default())?;
    assert_eq!(projected.text.title, "final");
    assert_eq!(projected.text.description, "kept");
    let empty = story(r#"[{"image":"assets/a"}]"#, r#", "title":"\v""#);
    assert_eq!(
        project_studio_json(&supplied_files()?, &empty, Limits::default()).map_err(|e| e.code),
        Err(ValidationCode::MalformedMetadata)
    );
    let bytes = story(
        r#"[{"image":"assets/a"}]"#,
        r#", "title":" \t ","description":null"#,
    );
    assert_eq!(
        project_studio_json(&supplied_files()?, &bytes, Limits::default())?.text,
        MetadataText::default()
    );
    for extra in [
        r#", "title":17,"title":" repaired ""#,
        r#", "description":false,"description":null"#,
    ] {
        let mut bytes = story(r#"[{"image":"assets/a"}]"#, extra);
        bytes.extend_from_slice(b" trailing");
        assert!(matches!(decode(&bytes), Err(DecodeFailure::Type { .. })));
        assert_eq!(
            project_studio_json(&supplied_files()?, &bytes, Limits::default()).map_err(|e| e.code),
            Err(ValidationCode::MalformedMetadata)
        );
    }
    Ok(())
}

#[test]
fn studio_text_replacement_origins_and_interior_composition_are_preserved() -> TestResult {
    let mut bytes = story(r#"[{"image":"assets/a"}]"#, "");
    bytes.pop();
    bytes.extend_from_slice(b",\"title\":\" \xed\xa0\x80 \",\"description\":\" \\uD800 \"}");
    let text = project_studio_json(&supplied_files()?, &bytes, Limits::default())?.text;
    assert_eq!(text.title, "\u{fffd}\u{fffd}\u{fffd}");
    assert_eq!(text.description, "\u{fffd}");
    let bytes = story(
        r#"[{"image":"assets/a"}]"#,
        r#", "title":" é e\u0301 ","description":"\uD83D\uDE00""#,
    );
    let text = project_studio_json(&supplied_files()?, &bytes, Limits::default())?.text;
    assert_eq!(text.title, "é e\u{0301}");
    assert_eq!(text.description, "😀");
    Ok(())
}

#[test]
fn studio_text_projection_follows_uuid_refs_regular_thumbnail_presence() -> TestResult {
    let empty = inventory(&[])?;
    for (bytes, code) in [
        (b"null".to_vec(), ValidationCode::InvalidUuid),
        (
            story("[]", r#", "title":" good ""#),
            ValidationCode::MissingAsset,
        ),
        (
            story(r#"[{"image":"../bad"}]"#, ""),
            ValidationCode::UnsafePath,
        ),
        (valid(), ValidationCode::MissingAsset),
    ] {
        assert_eq!(
            project_studio_json(&empty, &bytes, Limits::default()).map_err(|e| e.code),
            Err(code)
        );
    }
    let mut builder = InventoryBuilder::new(Limits::default(), CompressionAccounting::ZipEntries)?;
    for (name, is_directory) in [("assets/a", false), ("thumbnail.png", true)] {
        builder.push(EntryInfo {
            name: name.as_bytes(),
            compressed_bytes: 1,
            uncompressed_bytes: 1,
            is_directory,
            is_link: false,
        })?;
    }
    assert_eq!(
        project_studio_json(&builder.finish()?, &valid(), Limits::default()).map_err(|e| e.code),
        Err(ValidationCode::MissingEntry)
    );
    Ok(())
}
