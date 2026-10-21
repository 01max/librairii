use librairii_core::tagging::{normalize_color, normalize_key, normalize_label};
#[test]
fn frozen_tag_identity_keeps_simple_lowercase_distinct_from_search_folding() {
    for (input, expected) in [
        ("Été favorites", "ete-favorites"),
        ("Straße", "straße"),
        ("𝑨", "A"),
        ("İIıi", "iiıi"),
        ("a\u{2003}b", "a-b"),
        ("三٣", "三٣"),
        ("Persian ۰۱۲۳۴۵۶۷۸۹", "persian-۰۱۲۳۴۵۶۷۸۹"),
        ("NKo ߀߁߂߃߄߅߆߇߈߉", "nko-߀߁߂߃߄߅߆߇߈߉"),
        ("a\u{0301}b", "ab"),
        ("one___two", "one-two"),
    ] {
        assert_eq!(normalize_key(input).as_deref(), Ok(expected));
    }
    assert!(normalize_key("!!!").is_err());
    assert!(normalize_key(&"a".repeat(65)).is_err());
    assert!(normalize_label(&"a".repeat(81)).is_err());
    assert_eq!(normalize_color(" #267d5d ").as_deref(), Ok("#267D5D"));
    assert!(normalize_color("#+A0000").is_err());
}
#[test]
fn long_mark_input_has_no_saved_query_wire_limit_for_tag_identity() {
    let value = format!("a{}", "\u{0301}".repeat(524_288));
    assert_eq!(normalize_key(&value).as_deref(), Ok("a"));
}
#[test]
fn every_unicode_scalar_matches_the_actual_frozen_go_tag_key_digest()
-> Result<(), Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../librairii-storage/tests/fixtures/tags/normalization.json"
    ))?;
    assert_eq!(reference["go"], "go1.26.5");
    assert_eq!(reference["unicode"], "15.0.0");
    let mut hash = Sha256::new();
    for code in 0..=0x10ffff {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        let result = normalize_key(&c.to_string());
        let valid = result.is_ok();
        let key = result.unwrap_or_default();
        hash.update(code.to_le_bytes());
        hash.update((key.len() as u32).to_le_bytes());
        hash.update([u8::from(valid)]);
        hash.update(key.as_bytes());
    }
    assert_eq!(
        reference["scalarDigest"],
        librairii_core::Checksum::from_bytes(hash.finalize().into()).to_string()
    );
    for case in reference["cases"].as_array().ok_or("cases")? {
        let result = normalize_key(case["input"].as_str().ok_or("input")?);
        assert_eq!(result.is_ok(), case["valid"].as_bool().ok_or("valid")?);
        assert_eq!(
            result.unwrap_or_default(),
            case["key"].as_str().ok_or("key")?
        );
    }
    assert_eq!(
        reference["longMarks"]["key"],
        normalize_key(&format!("a{}", "\u{0301}".repeat(524_288)))?
    );
    Ok(())
}
