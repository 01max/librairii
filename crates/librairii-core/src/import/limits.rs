use super::{ValidationCode, ValidationError, validate_entry_name};

/// Frozen Go policy limits. These counters do not constrain decoder allocation,
/// constructor work, actual reads or already-materialized caller data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub max_path_bytes: usize,
    pub max_path_depth: usize,
    pub max_entries: usize,
    pub max_expanded_bytes: i64,
    pub max_compression_ratio: f64,
    pub max_metadata_bytes: i64,
    pub max_artwork_bytes: i64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_path_bytes: 512,
            max_path_depth: 16,
            max_entries: 10_000,
            max_expanded_bytes: 2 << 30,
            max_compression_ratio: 200.0,
            max_metadata_bytes: 1 << 20,
            max_artwork_bytes: 10 << 20,
        }
    }
}
impl Limits {
    pub fn validate(self) -> Result<(), ValidationError> {
        if self.max_path_bytes == 0
            || self.max_path_depth == 0
            || self.max_entries == 0
            || self.max_expanded_bytes <= 0
            || self.max_metadata_bytes <= 0
            || self.max_artwork_bytes <= 0
            || self.max_compression_ratio < 1.0
            || !self.max_compression_ratio.is_finite()
        {
            return Err(ValidationError::new(ValidationCode::InvalidLimits));
        }
        Ok(())
    }
}

/// Supplied header data, not observed decoded bytes or an owned file handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryInfo<'a> {
    pub name: &'a [u8],
    pub compressed_bytes: i64,
    pub uncompressed_bytes: i64,
    pub is_directory: bool,
    pub is_link: bool,
}
impl<'a> EntryInfo<'a> {
    /// Reject unsigned declarations outside Go's supported signed range before
    /// converting. The future adapter must not wrap u64 sizes into i64.
    pub fn from_unsigned(
        name: &'a [u8],
        compressed: u64,
        uncompressed: u64,
        is_directory: bool,
        is_link: bool,
    ) -> Result<Self, ValidationError> {
        let invalid = || {
            ValidationError::at(ValidationCode::InvalidContainer, name)
                .caused_by("entry size exceeds supported range")
        };
        Ok(Self {
            name,
            compressed_bytes: i64::try_from(compressed).map_err(|_| invalid())?,
            uncompressed_bytes: i64::try_from(uncompressed).map_err(|_| invalid())?,
            is_directory,
            is_link,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BudgetUsage {
    pub entries: usize,
    pub expanded_bytes: i64,
    pub compressed_bytes: i64,
}

/// Pure declared-size accounting. Failed updates leave usage unchanged; an
/// inventory builder is terminal after failure. Neither property is custody.
#[derive(Clone, Debug)]
pub struct Budget {
    limits: Limits,
    usage: BudgetUsage,
}
impl Budget {
    pub fn new(limits: Limits) -> Result<Self, ValidationError> {
        limits.validate()?;
        Ok(Self {
            limits,
            usage: BudgetUsage::default(),
        })
    }
    pub const fn usage(&self) -> BudgetUsage {
        self.usage
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    fn expanded_usage(&self, entry: EntryInfo<'_>) -> Result<BudgetUsage, ValidationError> {
        validate_entry_name(entry.name, self.limits)?;
        if entry.is_link {
            return Err(ValidationError::at(ValidationCode::LinkEntry, entry.name));
        }
        if entry.compressed_bytes < 0 || entry.uncompressed_bytes < 0 {
            return Err(
                ValidationError::at(ValidationCode::InvalidContainer, entry.name)
                    .caused_by("negative entry size"),
            );
        }
        let count = self
            .usage
            .entries
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_entries)
            .ok_or_else(|| ValidationError::at(ValidationCode::EntryLimit, entry.name))?;
        if entry.uncompressed_bytes > self.limits.max_expanded_bytes - self.usage.expanded_bytes {
            return Err(ValidationError::at(
                ValidationCode::ExpandedSizeLimit,
                entry.name,
            ));
        }
        Ok(BudgetUsage {
            entries: count,
            expanded_bytes: self.usage.expanded_bytes + entry.uncompressed_bytes,
            compressed_bytes: self.usage.compressed_bytes,
        })
    }
    /// ZIP denominator is the aggregate supplied per-entry compressed size.
    pub fn account(&mut self, entry: EntryInfo<'_>) -> Result<(), ValidationError> {
        let mut next = self.expanded_usage(entry)?;
        next.compressed_bytes = next
            .compressed_bytes
            .checked_add(entry.compressed_bytes)
            .ok_or_else(|| {
                ValidationError::at(ValidationCode::InvalidContainer, entry.name)
                    .caused_by("compressed size overflow")
            })?;
        self.check_ratio(next.expanded_bytes, next.compressed_bytes, Some(entry.name))?;
        self.usage = next;
        Ok(())
    }
    /// 7z accounts all declarations, then uses whole-container bytes separately.
    pub fn account_expanded(&mut self, entry: EntryInfo<'_>) -> Result<(), ValidationError> {
        self.usage = self.expanded_usage(entry)?;
        Ok(())
    }
    pub fn validate_compression(&self, whole_container_bytes: i64) -> Result<(), ValidationError> {
        self.check_ratio(self.usage.expanded_bytes, whole_container_bytes, None)
    }
    fn check_ratio(
        &self,
        expanded: i64,
        compressed: i64,
        entry: Option<&[u8]>,
    ) -> Result<(), ValidationError> {
        let error = |code| match entry {
            Some(name) => ValidationError::at(code, name),
            None => ValidationError::new(code),
        };
        if compressed < 0 {
            return Err(
                error(ValidationCode::InvalidContainer).caused_by("negative compressed size")
            );
        }
        let denominator = if compressed == 0 && expanded > 0 {
            1
        } else {
            compressed
        };
        // Intentional Go float64 ratio semantics, including 0/0 for no output.
        if (expanded as f64) / (denominator as f64) > self.limits.max_compression_ratio {
            return Err(error(ValidationCode::CompressionRatioLimit));
        }
        Ok(())
    }
}
