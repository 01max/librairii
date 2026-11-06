#[test]
fn every_unicode_scalar_matches_frozen_go_collection_name_identity()
-> Result<(), Box<dyn std::error::Error>> {
    use librairii_core::collections::normalize_name;
    use sha2::{Digest, Sha256};
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../librairii-storage/tests/fixtures/collections/reference.json"
    ))?;
    let mut h = Sha256::new();
    for code in 0..=0x10ffff {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        let result = normalize_name(&c.to_string());
        let valid = result.is_ok();
        let (name, key) = result.unwrap_or_default();
        h.update(code.to_le_bytes());
        h.update((name.len() as u32).to_le_bytes());
        h.update((key.len() as u32).to_le_bytes());
        h.update([u8::from(valid)]);
        h.update(name.as_bytes());
        h.update(key.as_bytes());
    }
    assert_eq!(
        reference["scalarDigest"],
        librairii_core::Checksum::from_bytes(h.finalize().into()).to_string()
    );
    for case in reference["cases"].as_array().ok_or("cases")? {
        let result = normalize_name(case["input"].as_str().ok_or("input")?);
        assert_eq!(result.is_ok(), case["valid"].as_bool().ok_or("valid")?);
        let (name, key) = result.unwrap_or_default();
        assert_eq!(case["name"], name);
        assert_eq!(case["normalizedName"], key);
    }
    Ok(())
}
