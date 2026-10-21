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
#[test]
fn cancellation_after_staging_rejects_but_cancellation_inside_activation_finishes_report()
-> Result<(), Box<dyn std::error::Error>> {
    use librairii_app::metadata::*;
    use librairii_core::{CatalogSnapshotId, JobId};
    use std::cell::Cell;
    struct Repo {
        cancel: Cell<bool>,
        during_activation: bool,
        failed: Cell<bool>,
    }
    impl CatalogRepository for Repo {
        fn begin(&self, _: JobId, _: Locale, _: &str) -> Result<(), RepositoryFailure> {
            Ok(())
        }
        fn stage(
            &self,
            _: JobId,
            _: NormalizedCatalog,
            _: &str,
        ) -> Result<CatalogSnapshotId, RepositoryFailure> {
            if !self.during_activation {
                self.cancel.set(true);
            }
            CatalogSnapshotId::new(1).map_err(|_| RepositoryFailure::Proven)
        }
        fn activate(
            &self,
            id: CatalogSnapshotId,
            at: &str,
        ) -> Result<RefreshReport, RepositoryFailure> {
            self.cancel.set(true);
            Ok(RefreshReport {
                sync: JobId::from_bytes([1; 16]),
                snapshot: id,
                matched_stories: 2,
                activated_at: at.into(),
            })
        }
        fn fail(
            &self,
            _: JobId,
            id: Option<CatalogSnapshotId>,
            code: RefreshCode,
            _: &str,
        ) -> Result<(), RepositoryFailure> {
            assert!(id.is_some());
            assert_eq!(code, RefreshCode::Cancelled);
            self.failed.set(true);
            Ok(())
        }
    }
    for during_activation in [false, true] {
        let repo = Repo {
            cancel: Cell::new(false),
            during_activation,
            failed: Cell::new(false),
        };
        let session = RefreshSession::begin(
            &repo,
            JobId::from_bytes([1; 16]),
            "en-GB",
            "2026-10-21T12:00:00Z",
        )
        .map_err(|e| format!("{e:?}"))?;
        let result = session.finish(Ok(RAW.to_vec()), "2026-10-21T12:00:01Z", || {
            repo.cancel.get()
        });
        if during_activation {
            assert!(result.is_ok());
            assert!(!repo.failed.get());
        } else {
            assert_eq!(result, Err(RefreshCode::Cancelled));
            assert!(repo.failed.get());
        }
    }
    Ok(())
}
