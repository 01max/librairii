//! In-process archive observation over an already owned, seekable source.
//!
//! Opening establishes a bounded index and dispatch plan. It does not establish
//! source custody, complete inspection, payload integrity or database admission.
use librairii_app::native_jobs::Cancellation;
use librairii_core::import::{
    ArchivePlan, CompressionAccounting, ContainerKind, EntryInfo, ImportIdentity, Inventory,
    InventoryBuilder, Limits, PackStructure, ValidationCode, ValidationError, dispatch,
    validate_entry_name, validate_plain_entries, validate_uuid_root_entries,
};
use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Read, Seek, SeekFrom},
    sync::Mutex,
};

/// Additional decoder bounds are separate from the preserved compatibility limits.
/// They require the qualification/hardening evidence in section-05 before admission.
const HEADER_BYTES: u64 = 16 * 1024 * 1024;
const DECODER_BYTES: u64 = 256 * 1024 * 1024;
const ZIP_BUFFER: usize = 256 * 1024;
type Result<T> = std::result::Result<T, ArchiveError>;

#[derive(Debug)]
pub enum ArchiveError {
    Validation(ValidationError),
    Cancelled,
}
impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(e) => e.fmt(f),
            Self::Cancelled => f.write_str("archive inspection cancelled"),
        }
    }
}
impl std::error::Error for ArchiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(e) => Some(e),
            Self::Cancelled => None,
        }
    }
}
impl From<ValidationError> for ArchiveError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
fn invalid() -> ArchiveError {
    ValidationError::new(ValidationCode::InvalidContainer).into()
}
fn checkpoint(cancel: &Cancellation) -> Result<()> {
    if cancel.is_cancelled() {
        Err(ArchiveError::Cancelled)
    } else {
        Ok(())
    }
}
#[derive(Debug)]
struct Checked<R> {
    source: R,
    cancel: Cancellation,
}
impl<R: Read> Read for Checked<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        // Interrupted is deliberately avoided: Read::read_exact retries it.
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("archive cancelled"));
        }
        self.source.read(bytes)
    }
}
impl<R: Seek> Seek for Checked<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("archive cancelled"));
        }
        self.source.seek(position)
    }
}
#[derive(Debug)]
struct Positioned<R>(Mutex<Checked<R>>);
impl<R: Read + Seek> rawzip::ReaderAt for Positioned<R> {
    fn read_at(&self, bytes: &mut [u8], offset: u64) -> io::Result<usize> {
        let mut source = self
            .0
            .lock()
            .map_err(|_| io::Error::other("archive source unavailable"))?;
        source.seek(SeekFrom::Start(offset))?;
        source.read(bytes)
    }
}
#[derive(Clone, Copy, Debug)]
struct ZipMember {
    location: rawzip::ZipArchiveEntryWayfinder,
    method: rawzip::CompressionMethod,
    encrypted: bool,
}
enum Reader<R: Read + Seek> {
    Zip {
        archive: rawzip::ZipArchive<Positioned<R>>,
        members: BTreeMap<String, ZipMember>,
    },
    Seven(Box<sevenz_turbo::ArchiveReader<Checked<R>>>),
}
/// Owned parser session; no archive entries are extracted to the filesystem.
pub struct Archive<R: Read + Seek> {
    reader: Reader<R>,
    inventory: Inventory,
    plan: ArchivePlan,
    cancel: Cancellation,
}
impl<R: Read + Seek> fmt::Debug for Archive<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Archive")
            .field("plan", &self.plan)
            .field("usage", &self.inventory.usage())
            .finish_non_exhaustive()
    }
}
impl<R: Read + Seek> Archive<R> {
    pub fn open(source: R, filename: &[u8], limits: Limits, cancel: &Cancellation) -> Result<Self> {
        limits.validate()?;
        let frozen = Limits::default();
        if limits.max_entries > frozen.max_entries
            || limits.max_path_bytes > frozen.max_path_bytes
            || limits.max_path_depth > frozen.max_path_depth
            || limits.max_expanded_bytes > frozen.max_expanded_bytes
            || limits.max_metadata_bytes > frozen.max_metadata_bytes
            || limits.max_artwork_bytes > frozen.max_artwork_bytes
            || limits.max_compression_ratio > frozen.max_compression_ratio
        {
            return Err(ValidationError::new(ValidationCode::InvalidLimits).into());
        }
        checkpoint(cancel)?;
        // Ask the existing suffix policy, without reading content, which family to open.
        let kind = if dispatch(
            filename,
            librairii_core::import::ContentFacts {
                container: ContainerKind::SevenZip,
                has_story_json: false,
                has_uuid_bin: false,
            },
        )
        .is_ok()
        {
            ContainerKind::SevenZip
        } else {
            dispatch(
                filename,
                librairii_core::import::ContentFacts {
                    container: ContainerKind::Zip,
                    has_story_json: false,
                    has_uuid_bin: false,
                },
            )?;
            ContainerKind::Zip
        };
        let mut source = Checked {
            source,
            cancel: cancel.clone(),
        };
        let size = source.seek(SeekFrom::End(0)).map_err(|_| invalid())?;
        source.seek(SeekFrom::Start(0)).map_err(|_| invalid())?;
        let opened = if kind == ContainerKind::Zip {
            Self::open_zip(source, limits, size)
        } else {
            Self::open_seven(source, limits, size)
        };
        checkpoint(cancel)?;
        let (reader, inventory) = opened?;
        let plan = dispatch(filename, inventory.content_facts(kind))?;
        Ok(Self {
            reader,
            inventory,
            plan,
            cancel: cancel.clone(),
        })
    }
    fn open_zip(source: Checked<R>, limits: Limits, size: u64) -> Result<(Reader<R>, Inventory)> {
        let mut buffer = vec![0; ZIP_BUFFER];
        let archive = rawzip::ZipLocator::new()
            .locate_in_reader(Positioned(Mutex::new(source)), &mut buffer, size)
            .map_err(|_| invalid())?;
        let mut index = InventoryBuilder::new(limits, CompressionAccounting::ZipEntries)?;
        let mut members = BTreeMap::new();
        let mut entries = archive.entries(&mut buffer);
        let mut count = 0u64;
        while let Some(entry) = entries.next_entry().map_err(|_| invalid())? {
            let path = entry.file_path();
            let name = path.as_bytes();
            index.push(EntryInfo::from_unsigned(
                name,
                entry.compressed_size_hint(),
                entry.uncompressed_size_hint(),
                entry.is_dir() || entry.mode().value() & 0xf000 == 0x4000,
                entry.mode().is_symlink(),
            )?)?;
            let key = validate_entry_name(name, limits)?;
            members.insert(
                key,
                ZipMember {
                    location: entry.wayfinder(),
                    method: entry.compression_method(),
                    encrypted: entry.flags().is_encrypted()
                        || entry.flags().has_strong_encryption()
                        || entry.flags().is_masked(),
                },
            );
            count += 1;
        }
        if count != archive.entries_hint() {
            return Err(invalid());
        }
        Ok((Reader::Zip { archive, members }, index.finish()?))
    }
    fn open_seven(source: Checked<R>, limits: Limits, size: u64) -> Result<(Reader<R>, Inventory)> {
        let decoder_limits = sevenz_turbo::ArchiveLimits {
            memory_limit_bytes: DECODER_BYTES,
            max_end_header_bytes: HEADER_BYTES,
            max_header_unpacked_bytes: HEADER_BYTES,
            max_entries: limits.max_entries as u64,
            // This is UTF-16 storage size; the compatibility path cap is UTF-8.
            max_name_bytes: limits.max_path_bytes.saturating_mul(2) as u64,
            max_total_name_bytes: limits
                .max_entries
                .saturating_mul(limits.max_path_bytes)
                .saturating_mul(2) as u64,
            max_total_coders: limits.max_entries as u64,
            max_unpack_bytes: limits.max_expanded_bytes as u64,
            // Frozen ratio uses the entire container, not compressed stream sizes.
            max_unpack_ratio: u64::MAX,
            reject_unsafe_paths: false,
            ..Default::default()
        };
        let reader = sevenz_turbo::ArchiveReader::with_limits(
            source,
            sevenz_turbo::Password::empty(),
            decoder_limits,
        )
        .map_err(seven_error)?;
        let mut index = InventoryBuilder::new(
            limits,
            CompressionAccounting::SevenZipContainer {
                bytes: i64::try_from(size).map_err(|_| invalid())?,
            },
        )?;
        for entry in &reader.archive().files {
            let mode = entry.unix_mode().unwrap_or(0) & 0xf000;
            let special = entry.is_symlink()
                || (mode != 0 && mode != 0x8000 && mode != 0x4000)
                || entry.is_anti_item();
            index.push(EntryInfo::from_unsigned(
                entry.name().as_bytes(),
                0,
                entry.size(),
                entry.is_directory(),
                special,
            )?)?;
        }
        Ok((Reader::Seven(Box::new(reader)), index.finish()?))
    }
    pub const fn plan(&self) -> ArchivePlan {
        self.plan
    }
    pub const fn inventory(&self) -> &Inventory {
        &self.inventory
    }
    /// Validate a pack's required files and actual UUID bytes. Full inspection
    /// still needs optional metadata/artwork and the family's payload checks.
    pub fn inspect_pack_identity(&mut self) -> Result<ImportIdentity> {
        checkpoint(&self.cancel)?;
        match self.plan.structure {
            PackStructure::Plain => {
                validate_plain_entries(&self.inventory)?;
                let bytes = self.read_entry("uuid.bin", 16, ValidationCode::InvalidUuid)?;
                Ok(ImportIdentity::from_plain_bytes(&bytes)?)
            }
            PackStructure::UuidRoot => Ok(validate_uuid_root_entries(&self.inventory)?.identity),
            PackStructure::Studio => {
                Err(ValidationError::new(ValidationCode::UnsupportedFormat).into())
            }
        }
    }
    /// Read one selected regular entry under its own actual-byte ceiling.
    pub fn read_entry(&mut self, name: &str, limit: u64, code: ValidationCode) -> Result<Vec<u8>> {
        checkpoint(&self.cancel)?;
        let record = self
            .inventory
            .entry(name)
            .filter(|e| !e.is_directory)
            .ok_or_else(|| ValidationError::at(ValidationCode::MissingEntry, name))?;
        let raw_name = record.raw_name.clone();
        let result = match &mut self.reader {
            Reader::Zip { archive, members } => {
                let member = members.get(name).ok_or_else(invalid)?;
                if member.encrypted {
                    return Err(invalid());
                }
                let entry = archive.get_entry(member.location).map_err(|_| invalid())?;
                let decoder: Box<dyn Read + '_> =
                    if member.method == rawzip::CompressionMethod::STORE {
                        Box::new(entry.reader())
                    } else if member.method == rawzip::CompressionMethod::DEFLATE {
                        Box::new(flate2::read::DeflateDecoder::new(entry.reader()))
                    } else {
                        return Err(invalid());
                    };
                limited(
                    &mut entry.verifying_reader(decoder),
                    limit,
                    code,
                    &raw_name,
                    &self.cancel,
                )
            }
            Reader::Seven(reader) => {
                let mut selected = None;
                let decoded = reader.for_each_entries(|entry, bytes| {
                    if entry.name().as_bytes() != raw_name {
                        return Ok(true);
                    }
                    selected = Some(limited(bytes, limit, code, &raw_name, &self.cancel));
                    Ok(false)
                });
                if let Err(error) = decoded {
                    Err(seven_error(error))
                } else {
                    selected.unwrap_or_else(|| Err(invalid()))
                }
            }
        };
        checkpoint(&self.cancel)?;
        result
    }
}
fn limited(
    reader: &mut dyn Read,
    limit: u64,
    code: ValidationCode,
    name: &[u8],
    cancel: &Cancellation,
) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut buffer = [0; 32 * 1024];
    loop {
        checkpoint(cancel)?;
        let available = limit
            .saturating_sub(result.len() as u64)
            .saturating_add(1)
            .min(buffer.len() as u64) as usize;
        let count = reader
            .read(&mut buffer[..available])
            .map_err(|_| ValidationError::at(code, name))?;
        if count == 0 {
            return Ok(result);
        }
        if count as u64 > limit - result.len() as u64 {
            return Err(ValidationError::at(code, name).into());
        }
        result
            .try_reserve(count)
            .map_err(|_| ValidationError::at(code, name))?;
        result.extend_from_slice(&buffer[..count]);
    }
}

fn seven_error(error: sevenz_turbo::Error) -> ArchiveError {
    use sevenz_turbo::{Error, Limit};
    let code = match error {
        Error::LimitExceeded {
            what: Limit::Entries,
            ..
        } => ValidationCode::EntryLimit,
        Error::LimitExceeded {
            what: Limit::NameBytes,
            ..
        } => ValidationCode::UnsafePath,
        Error::LimitExceeded {
            what: Limit::UnpackBytes,
            ..
        } => ValidationCode::ExpandedSizeLimit,
        _ => ValidationCode::InvalidContainer,
    };
    ValidationError::new(code).into()
}
#[cfg(test)]
mod tests;
