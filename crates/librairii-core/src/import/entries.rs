use super::{
    Budget, BudgetUsage, ContainerKind, ContentFacts, EntryInfo, Limits, ValidationCode,
    ValidationError, suffix_matches,
};
use std::collections::BTreeMap;

/// Return the lexical Go path.Clean lookup key. Original name bytes are not
/// rewritten; this key is not a filesystem path, Unicode normalization or proof
/// a native path can be published safely.
pub fn validate_entry_name(name: &[u8], limits: Limits) -> Result<String, ValidationError> {
    limits.validate()?;
    let unsafe_name = || ValidationError::at(ValidationCode::UnsafePath, name);
    if name.is_empty()
        || name.len() > limits.max_path_bytes
        || name.contains(&0)
        || name.contains(&b'\\')
    {
        return Err(unsafe_name());
    }
    let text = std::str::from_utf8(name).map_err(|_| unsafe_name())?;
    if text.starts_with('/')
        || (name.len() >= 2 && name[0].is_ascii_alphabetic() && name[1] == b':')
    {
        return Err(unsafe_name());
    }
    let mut components = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." if components.last().is_some_and(|last| *last != "..") => {
                components.pop();
            }
            _ => components.push(part),
        }
    }
    if components.is_empty() || components[0] == ".." || components.len() > limits.max_path_depth {
        return Err(unsafe_name());
    }
    Ok(components.join("/"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompressionAccounting {
    ZipEntries,
    SevenZipContainer { bytes: i64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryRecord {
    pub raw_name: Vec<u8>,
    pub compressed_bytes: i64,
    pub uncompressed_bytes: i64,
    pub is_directory: bool,
}

/// Indexed supplied data only. No archive was opened, CRC checked, source hash
/// verified or decoder budget enforced by constructing this value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inventory {
    entries: BTreeMap<String, EntryRecord>,
    usage: BudgetUsage,
}
impl Inventory {
    pub fn entry(&self, key: &str) -> Option<&EntryRecord> {
        self.entries.get(key)
    }
    pub fn has_file(&self, key: &str) -> bool {
        self.entry(key).is_some_and(|entry| !entry.is_directory)
    }
    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|(_, entry)| !entry.is_directory)
            .map(|(name, _)| name.as_str())
    }
    pub fn has_file_below(&self, prefix: &str) -> bool {
        let prefix = format!("{}/", prefix.strip_suffix('/').unwrap_or(prefix));
        self.file_names().any(|name| name.starts_with(&prefix))
    }
    pub const fn usage(&self) -> BudgetUsage {
        self.usage
    }
    pub fn content_facts(&self, container: ContainerKind) -> ContentFacts {
        ContentFacts {
            container,
            has_story_json: self.has_file("story.json"),
            has_uuid_bin: self.has_file("uuid.bin"),
        }
    }
}

/// Terminal-on-error inventory policy. A caller must not keep a partial
/// inventory after a rejected entry and present it as a complete source.
#[derive(Clone, Debug)]
pub struct InventoryBuilder {
    budget: Budget,
    accounting: CompressionAccounting,
    entries: BTreeMap<String, EntryRecord>,
    failure: Option<ValidationError>,
}
impl InventoryBuilder {
    pub fn new(limits: Limits, accounting: CompressionAccounting) -> Result<Self, ValidationError> {
        Ok(Self {
            budget: Budget::new(limits)?,
            accounting,
            entries: BTreeMap::new(),
            failure: None,
        })
    }
    pub fn push(&mut self, entry: EntryInfo<'_>) -> Result<(), ValidationError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.push_checked(entry);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    fn push_checked(&mut self, entry: EntryInfo<'_>) -> Result<(), ValidationError> {
        match self.accounting {
            CompressionAccounting::ZipEntries => self.budget.account(entry)?,
            CompressionAccounting::SevenZipContainer { .. } => {
                self.budget.account_expanded(entry)?
            }
        }
        let key = validate_entry_name(entry.name, self.budget.limits())?;
        if self.entries.contains_key(&key) {
            return Err(ValidationError::at(
                ValidationCode::AmbiguousStructure,
                entry.name,
            ));
        }
        if !entry.is_directory
            && (suffix_matches(key.as_bytes(), ".zip")
                || suffix_matches(key.as_bytes(), ".7z")
                || suffix_matches(key.as_bytes(), ".pk"))
        {
            return Err(ValidationError::at(
                ValidationCode::NestedArchive,
                entry.name,
            ));
        }
        self.entries.insert(
            key,
            EntryRecord {
                raw_name: entry.name.to_vec(),
                compressed_bytes: entry.compressed_bytes,
                uncompressed_bytes: entry.uncompressed_bytes,
                is_directory: entry.is_directory,
            },
        );
        Ok(())
    }
    pub fn finish(self) -> Result<Inventory, ValidationError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if let CompressionAccounting::SevenZipContainer { bytes } = self.accounting {
            self.budget.validate_compression(bytes)?;
        }
        Ok(Inventory {
            entries: self.entries,
            usage: self.budget.usage(),
        })
    }
}
