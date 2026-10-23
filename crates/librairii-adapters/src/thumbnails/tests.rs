use super::*;
use std::error::Error;
type TestResult = std::result::Result<(), Box<dyn Error>>;
fn asset() -> Result<ArtworkAsset> {
    ArtworkAsset::checked(
        include_bytes!("../../tests/fixtures/artwork/cover.png").to_vec(),
        MediaType::Png,
    )
}
#[test]
fn thumbnails_decode_all_formats_reuse_lru_and_bound_pixels_and_output() -> TestResult {
    let cache = ThumbnailCache::default();
    let generation = cache.generation()?;
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        600,
        300,
        image::Rgb([23, 45, 67]),
    ));
    for (format, media) in [
        (ImageFormat::Png, MediaType::Png),
        (ImageFormat::Jpeg, MediaType::Jpeg),
        (ImageFormat::WebP, MediaType::Webp),
    ] {
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format)?;
        let asset = ArtworkAsset::checked(bytes.into_inner(), media)?;
        let first = cache.decode(&generation, &asset)?.ok_or("thumbnail")?;
        assert_eq!((first.width, first.height), (256, 128));
        assert_eq!(first.rgba.len(), 256 * 128 * 4);
        let second = cache.decode(&generation, &asset)?.ok_or("cache")?;
        assert!(Arc::ptr_eq(&first.rgba, &second.rgba));
    }
    let mut bomb = asset()?.bytes.to_vec();
    bomb[16..20].copy_from_slice(&(MAX_DIMENSION + 1).to_be_bytes());
    let crc = crc32(&bomb[12..29]);
    bomb[29..33].copy_from_slice(&crc.to_be_bytes());
    assert!(
        cache
            .decode(&generation, &ArtworkAsset::checked(bomb, MediaType::Png)?)
            .is_err()
    );
    let mut bomb = asset()?.bytes.to_vec();
    bomb[16..20].copy_from_slice(&MAX_DIMENSION.to_be_bytes());
    bomb[20..24].copy_from_slice(&MAX_DIMENSION.to_be_bytes());
    let crc = crc32(&bomb[12..29]);
    bomb[29..33].copy_from_slice(&crc.to_be_bytes());
    assert!(
        cache
            .decode(&generation, &ArtworkAsset::checked(bomb, MediaType::Png)?)
            .is_err()
    );
    Ok(())
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 1 { 0xedb88320 } else { 0 };
        }
    }
    !crc
}
#[test]
fn invalidation_rejects_old_work_and_fallback_is_official_then_embedded_then_placeholder()
-> TestResult {
    let cache = ThumbnailCache::default();
    let old = cache.generation()?;
    let good = asset()?;
    let bad = ArtworkAsset::checked(b"\x89PNG\r\n\x1a\ninvalid".to_vec(), MediaType::Png)?;
    assert!(matches!(
        cache.resolve(&old, Some(&good), None)?,
        ArtworkModel::Ready {
            source: ArtworkSource::Official,
            ..
        }
    ));
    assert!(matches!(
        cache.resolve(&old, Some(&bad), Some(&good))?,
        ArtworkModel::Ready {
            source: ArtworkSource::Embedded,
            ..
        }
    ));
    assert_eq!(
        cache.resolve(&old, Some(&bad), None)?,
        ArtworkModel::Placeholder
    );
    cache.invalidate()?;
    assert_eq!(cache.decode(&old, &good)?, None);
    assert_eq!(cache.resolve(&old, None, None)?, ArtworkModel::Stale);
    let new = cache.generation()?;
    assert!(cache.decode(&new, &good)?.is_some());
    cache.decoders.store(2, Ordering::Release);
    let unique = ArtworkAsset::checked(bad.bytes.to_vec(), MediaType::Png)?;
    assert_eq!(cache.decode(&new, &unique), Err(MetadataError::Busy));
    cache.decoders.store(0, Ordering::Release);
    // An in-flight result holding the old token cannot publish after reset.
    let before = cache.generation()?;
    cache.invalidate()?;
    assert!(cache.decode(&before, &good)?.is_none());
    Ok(())
}
#[test]
fn thumbnail_lru_evicts_old_results_at_the_entry_budget() -> TestResult {
    let cache = ThumbnailCache::default();
    let generation = cache.generation()?;
    let first = asset()?;
    let old = cache.decode(&generation, &first)?.ok_or("first")?;
    for index in 0..CACHE_ENTRIES {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            2,
            2,
            image::Rgb([index as u8, 0, 0]),
        ))
        .write_to(&mut bytes, ImageFormat::Png)?;
        let next = ArtworkAsset::checked(bytes.into_inner(), MediaType::Png)?;
        assert!(cache.decode(&generation, &next)?.is_some());
    }
    let state = cache.state.lock().map_err(|_| "lock")?;
    assert!(state.entries.len() <= CACHE_ENTRIES);
    assert!(state.bytes <= CACHE_BYTES);
    drop(state);
    let reloaded = cache.decode(&generation, &first)?.ok_or("reloaded")?;
    assert!(!Arc::ptr_eq(&old.rgba, &reloaded.rgba));
    Ok(())
}
