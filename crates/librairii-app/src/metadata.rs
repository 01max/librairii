//! Validated catalog data and explicit refresh use cases, independent of SQL,
//! filesystem handles, HTTP libraries and native presentation.
mod normalize;
use librairii_core::{
    Checksum,
    metadata::{CatalogArtwork, CatalogRecord, Locale, MetadataError},
};
pub use normalize::{NormalizedCatalog, normalize};
use std::sync::Arc;
type Result<T> = std::result::Result<T, MetadataError>;
pub const MAX_CATALOG_BYTES: usize = 32 * 1024 * 1024;
pub const ARTWORK_BASE: &str = "https://storage.googleapis.com/lunii-data-prod";
pub fn digest(bytes: &[u8]) -> Checksum {
    use sha2::Digest;
    Checksum::from_bytes(sha2::Sha256::digest(bytes).into())
}
/// Validate before URL libraries can normalize dot segments or escapes away.
pub fn artwork_url(raw_path: &str) -> Result<String> {
    if !raw_path.starts_with('/')
        || raw_path.starts_with("//")
        || raw_path.ends_with('/')
        || raw_path.contains(['\\', '?', '#', '%', '\0'])
        || raw_path.chars().any(char::is_control)
        || raw_path
            .split('/')
            .skip(1)
            .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return Err(MetadataError::InvalidArtwork);
    }
    Ok(format!("{ARTWORK_BASE}{raw_path}"))
}
/// Canonical UTC timestamp bytes use Go RFC3339Nano spelling.
pub fn utc_time(value: &str) -> Result<String> {
    let instant =
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
            .map_err(|_| MetadataError::InvalidCatalog)?;
    instant
        .to_offset(time::UtcOffset::UTC)
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| MetadataError::InvalidCatalog)
}
mod refresh;
pub use refresh::{
    CatalogRepository, RefreshCode, RefreshReport, RefreshSession, RepositoryFailure,
};
mod artwork;
pub use artwork::{
    ArtworkAsset, ArtworkRepository, MAX_ARTWORK_BYTES, MediaType, RegisteredArtwork,
    validate_artwork,
};
