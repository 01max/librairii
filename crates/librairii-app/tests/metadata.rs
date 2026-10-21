use librairii_app::metadata::{artwork_url, digest, normalize};
use librairii_core::metadata::{Locale, title_key};
use serde_json::{Value, json};
const RAW: &[u8] = include_bytes!("fixtures/lunii-catalog.json");
#[test]
fn exact_raw_hash_normalization_and_requested_same_language_available_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let c = normalize(RAW.to_vec(), "en_GB")?;
    assert_eq!(c.raw(), RAW);
    assert_eq!(c.checksum(), digest(RAW));
    assert_eq!(c.locale().as_str(), "en-GB");
    assert_eq!(c.records()[0].title, "The Clockwork Mountain");
    assert_eq!(c.records()[0].duration_seconds, Some(3240));
    assert_eq!(c.records()[0].minimum_age, Some(3));
    assert_eq!(c.records()[0].maximum_age, Some(5));
    let mut v: Value = serde_json::from_slice(RAW)?;
    let row = v["response"]
        .as_object_mut()
        .ok_or("response")?
        .values_mut()
        .next()
        .ok_or("row")?;
    row["locales_available"] = json!({"en_US":true,"fr_FR":true,"de_DE":false});
    row["localized_infos"] = json!({"en_US":{"title":"English"},"fr_FR":{"title":"Français"},"de_DE":{"title":"Unavailable"}});
    let bytes = serde_json::to_vec(&v)?;
    assert_eq!(
        normalize(bytes.clone(), "fr-FR")?.records()[0].title,
        "Français"
    );
    assert_eq!(
        normalize(bytes.clone(), "en-GB")?.records()[0]
            .language
            .as_str(),
        "en-US"
    );
    assert_eq!(normalize(bytes, "it-IT")?.records()[0].title, "English");
    assert_eq!(title_key(" ÉTÉ  --  Forêt ")?, "ete -- foret");
    assert_eq!(Locale::parse("iw_IL")?.as_str(), "he-IL");
    Ok(())
}
#[test]
fn invalid_catalogs_never_construct_normalized_snapshots() -> Result<(), Box<dyn std::error::Error>>
{
    for bytes in [
        b"null".to_vec(),
        b"{\"response\":{}}".to_vec(),
        b"{} trailing".to_vec(),
    ] {
        assert!(normalize(bytes, "en-GB").is_err());
    }
    let base: Value = serde_json::from_slice(RAW)?;
    for (field, value) in [
        ("uuid", json!("00000000-0000-0000-0000-000000000000")),
        ("locales_available", json!({"en_GB":true,"en-GB":true})),
        ("updated_at", json!("yesterday")),
    ] {
        let mut v = base.clone();
        v["response"]
            .as_object_mut()
            .ok_or("response")?
            .values_mut()
            .next()
            .ok_or("row")?[field] = value;
        assert!(
            normalize(serde_json::to_vec(&v)?, "en-GB").is_err(),
            "{field}"
        );
    }
    for path in [
        "https://outside/image.png",
        "//outside/image.png",
        "/a/../b",
        "/a/%2e/b",
        "/a?x",
        "/a\\b",
        "/a//b",
        "/",
    ] {
        assert!(artwork_url(path).is_err(), "{path}");
    }
    Ok(())
}
