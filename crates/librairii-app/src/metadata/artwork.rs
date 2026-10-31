use super::*;
pub const MAX_ARTWORK_BYTES: usize = 10 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaType {
    Png,
    Jpeg,
    Webp,
}
impl MediaType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "image/png" => Ok(Self::Png),
            "image/jpeg" => Ok(Self::Jpeg),
            "image/webp" => Ok(Self::Webp),
            _ => Err(MetadataError::InvalidArtwork),
        }
    }
}
/// Sniffing is a storage/transport check; bounded decoding is a separate step.
pub fn validate_artwork(bytes: &[u8], media: MediaType) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_ARTWORK_BYTES {
        return Err(MetadataError::TooLarge);
    }
    let actual = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(MediaType::Png)
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(MediaType::Jpeg)
    } else if bytes.len() >= 14 && &bytes[..4] == b"RIFF" && &bytes[8..14] == b"WEBPVP" {
        Some(MediaType::Webp)
    } else {
        None
    };
    if actual != Some(media) {
        return Err(MetadataError::InvalidArtwork);
    }
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtworkAsset {
    pub bytes: Arc<[u8]>,
    pub media: MediaType,
    pub checksum: Checksum,
}
impl ArtworkAsset {
    pub fn checked(bytes: Vec<u8>, media: MediaType) -> Result<Self> {
        validate_artwork(&bytes, media)?;
        Ok(Self {
            checksum: digest(&bytes),
            bytes: bytes.into(),
            media,
        })
    }
}
#[derive(Clone, Debug)]
pub struct RegisteredArtwork {
    pub id: Checksum,
    pub source_url: String,
    pub cached: Option<ArtworkAsset>,
}
/// The adapter loads only opaque registered identities; URLs never authorize
/// filesystem effects. Concrete repositories bind their own library runtime.
pub trait ArtworkRepository {
    fn registered_artwork(&self, id: Checksum) -> Result<RegisteredArtwork>;
    fn cache_artwork(
        &self,
        id: Checksum,
        asset: &ArtworkAsset,
        at: &str,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Result<()>;
}
impl<T: ArtworkRepository + ?Sized> ArtworkRepository for &T {
    fn registered_artwork(&self, id: Checksum) -> Result<RegisteredArtwork> {
        (**self).registered_artwork(id)
    }
    fn cache_artwork(
        &self,
        id: Checksum,
        asset: &ArtworkAsset,
        at: &str,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Result<()> {
        (**self).cache_artwork(id, asset, at, etag, last_modified)
    }
}
