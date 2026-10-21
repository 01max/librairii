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
