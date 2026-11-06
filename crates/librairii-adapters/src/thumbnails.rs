//! Bounded synchronous decode on a blocking worker, independent of native UI.
//! Keep one cache per open library. Invalidate at successful activation and
//! before root relocation; old work cannot insert or publish into a new epoch.
use image::{ImageDecoder, ImageFormat, ImageReader};
use librairii_app::metadata::{ArtworkAsset, MediaType, validate_artwork};
use librairii_core::{Checksum, metadata::MetadataError};
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
type Result<T> = std::result::Result<T, MetadataError>;
pub const THUMBNAIL_EDGE: u32 = 256;
pub const MAX_DIMENSION: u32 = 8192;
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
pub const CACHE_BYTES: usize = 32 * 1024 * 1024;
pub const CACHE_ENTRIES: usize = 128;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}
#[derive(Clone, Debug)]
pub struct Generation(Arc<()>);
#[derive(Debug)]
struct State {
    generation: Generation,
    entries: VecDeque<(Checksum, Thumbnail)>,
    bytes: usize,
}
#[derive(Debug)]
pub struct ThumbnailCache {
    state: Mutex<State>,
    decoders: AtomicUsize,
}
impl Default for ThumbnailCache {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                generation: Generation(Arc::new(())),
                entries: VecDeque::new(),
                bytes: 0,
            }),
            decoders: AtomicUsize::new(0),
        }
    }
}
struct DecoderSlot<'a>(&'a AtomicUsize);
impl Drop for DecoderSlot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}
impl ThumbnailCache {
    pub fn generation(&self) -> Result<Generation> {
        Ok(self
            .state
            .lock()
            .map_err(|_| MetadataError::InvalidArtwork)?
            .generation
            .clone())
    }
    pub fn invalidate(&self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| MetadataError::InvalidArtwork)?;
        state.generation = Generation(Arc::new(()));
        state.entries.clear();
        state.bytes = 0;
        Ok(())
    }
    pub fn decode(
        &self,
        generation: &Generation,
        asset: &ArtworkAsset,
    ) -> Result<Option<Thumbnail>> {
        validate_artwork(&asset.bytes, asset.media)?;
        if librairii_app::metadata::digest(&asset.bytes) != asset.checksum {
            return Err(MetadataError::InvalidArtwork);
        }
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| MetadataError::InvalidArtwork)?;
            if !Arc::ptr_eq(&generation.0, &state.generation.0) {
                return Ok(None);
            }
            if let Some(index) = state
                .entries
                .iter()
                .position(|(hash, _)| *hash == asset.checksum)
            {
                let entry = state
                    .entries
                    .remove(index)
                    .ok_or(MetadataError::InvalidArtwork)?;
                let thumbnail = entry.1.clone();
                state.entries.push_back(entry);
                return Ok(Some(thumbnail));
            }
        }
        self.decoders
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| MetadataError::Busy)?;
        let _slot = DecoderSlot(&self.decoders);
        let thumbnail = decode_bounded(asset)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| MetadataError::InvalidArtwork)?;
        if !Arc::ptr_eq(&generation.0, &state.generation.0) {
            return Ok(None);
        }
        if let Some(index) = state
            .entries
            .iter()
            .position(|(hash, _)| *hash == asset.checksum)
            && let Some((_, old)) = state.entries.remove(index)
        {
            state.bytes -= old.rgba.len();
        }
        while state.entries.len() >= CACHE_ENTRIES
            || state.bytes + thumbnail.rgba.len() > CACHE_BYTES
        {
            let Some((_, old)) = state.entries.pop_front() else {
                break;
            };
            state.bytes -= old.rgba.len();
        }
        state.bytes += thumbnail.rgba.len();
        state.entries.push_back((asset.checksum, thumbnail.clone()));
        Ok(Some(thumbnail))
    }
    /// Independently resolve official/embedded data. Cached official decode
    /// failure still permits a valid embedded cover or the native placeholder.
    pub fn resolve(
        &self,
        generation: &Generation,
        official: Option<&ArtworkAsset>,
        embedded: Option<&ArtworkAsset>,
    ) -> Result<ArtworkModel> {
        for (asset, source) in [
            (official, ArtworkSource::Official),
            (embedded, ArtworkSource::Embedded),
        ] {
            if let Some(asset) = asset {
                match self.decode(generation, asset) {
                    Ok(Some(thumbnail)) => return Ok(ArtworkModel::Ready { thumbnail, source }),
                    Ok(None) => return Ok(ArtworkModel::Stale),
                    Err(MetadataError::Busy) => return Err(MetadataError::Busy),
                    Err(_) => {}
                }
            }
        }
        let state = self
            .state
            .lock()
            .map_err(|_| MetadataError::InvalidArtwork)?;
        if !Arc::ptr_eq(&generation.0, &state.generation.0) {
            return Ok(ArtworkModel::Stale);
        }
        Ok(ArtworkModel::Placeholder)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtworkSource {
    Official,
    Embedded,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtworkModel {
    Ready {
        thumbnail: Thumbnail,
        source: ArtworkSource,
    },
    Placeholder,
    Stale,
}
fn decode_bounded(asset: &ArtworkAsset) -> Result<Thumbnail> {
    let format = match asset.media {
        MediaType::Png => ImageFormat::Png,
        MediaType::Jpeg => ImageFormat::Jpeg,
        MediaType::Webp => ImageFormat::WebP,
    };
    let mut reader = ImageReader::with_format(Cursor::new(asset.bytes.as_ref()), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|_| MetadataError::InvalidArtwork)?;
    let (w, h) = decoder.dimensions();
    if w == 0
        || h == 0
        || u64::from(w) * u64::from(h) > MAX_PIXELS
        || decoder.total_bytes() > 128 * 1024 * 1024
    {
        return Err(MetadataError::InvalidArtwork);
    }
    let image =
        image::DynamicImage::from_decoder(decoder).map_err(|_| MetadataError::InvalidArtwork)?;
    let rgba = image.thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE).into_rgba8();
    Ok(Thumbnail {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw().into(),
    })
}
#[cfg(test)]
mod tests;
