use crate::BoundaryError;
use std::{fmt, num::NonZeroI64, str::FromStr};
use uuid::Uuid;

macro_rules! uuid_identity {
    ($name:ident) => {
        /// Typed UUID bytes. Display is canonical diagnostic text, not a persisted codec.
        /// Storage must retain the original legacy representation alongside this value.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Uuid);
        impl $name {
            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }
            pub const fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }
        }
        impl FromStr for $name {
            type Err = BoundaryError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|_| BoundaryError::InvalidUuid)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
uuid_identity!(StoryUuid);
uuid_identity!(JobId);
uuid_identity!(LibraryId);

macro_rules! group_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroI64);
        impl $name {
            pub fn new(value: i64) -> Result<Self, BoundaryError> {
                NonZeroI64::new(value)
                    .filter(|v| v.get() > 0)
                    .map(Self)
                    .ok_or(BoundaryError::InvalidGroupId)
            }
            pub const fn get(self) -> i64 {
                self.0.get()
            }
        }
    };
}
group_identity!(CollectionId);
group_identity!(ShelfId);
group_identity!(CatalogSnapshotId);

/// SHA-256 identity only; hashing/custody is implemented in later sections.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Checksum([u8; 32]);
impl Checksum {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl FromStr for Checksum {
    type Err = BoundaryError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 {
            return Err(BoundaryError::InvalidChecksum);
        }
        let mut bytes = [0; 32];
        for (output, pair) in bytes.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
            let high = (pair[0] as char)
                .to_digit(16)
                .ok_or(BoundaryError::InvalidChecksum)?;
            let low = (pair[1] as char)
                .to_digit(16)
                .ok_or(BoundaryError::InvalidChecksum)?;
            *output = ((high << 4) | low) as u8;
        }
        Ok(Self(bytes))
    }
}
impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uuid_keeps_all_bytes_and_formats_canonically() -> Result<(), BoundaryError> {
        let value: StoryUuid = "AABBCCDD-1234-5678-9012-AABBCCDDEEFF".parse()?;
        assert_eq!(value.to_string(), "aabbccdd-1234-5678-9012-aabbccddeeff");
        assert_eq!(value, StoryUuid::from_bytes(*value.as_bytes()));
        assert_eq!(
            "not-a-uuid".parse::<StoryUuid>(),
            Err(BoundaryError::InvalidUuid)
        );
        Ok(())
    }
    #[test]
    fn group_ids_preserve_signed_database_range_and_reject_nonpositive_values()
    -> Result<(), BoundaryError> {
        assert_eq!(CollectionId::new(0), Err(BoundaryError::InvalidGroupId));
        assert_eq!(ShelfId::new(-1), Err(BoundaryError::InvalidGroupId));
        assert_eq!(CollectionId::new(i64::MAX)?.get(), i64::MAX);
        Ok(())
    }
    #[test]
    fn checksum_rejects_nonhex_wrong_length_and_multibyte_text() -> Result<(), BoundaryError> {
        let upper = "AB".repeat(32);
        let parsed: Checksum = upper.parse()?;
        assert_eq!(parsed.as_bytes(), &[0xab; 32]);
        assert_eq!(parsed.to_string(), "ab".repeat(32));
        for invalid in [
            "a".repeat(63),
            "a".repeat(65),
            "gg".repeat(32),
            "é".repeat(32),
        ] {
            assert_eq!(
                invalid.parse::<Checksum>(),
                Err(BoundaryError::InvalidChecksum)
            );
        }
        Ok(())
    }
}
