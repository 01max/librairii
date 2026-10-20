//! Storage-owned OS mechanisms; no domain repository or legacy-launch policy.
pub mod lock;

use crate::{Result, StorageError, database::WorkCancellation};
use librairii_core::Checksum;
use sha2::{Digest, Sha256};
use std::io::{self, Read};

pub const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// A complete byte measurement, not a file/cleanup/publication capability.
/// Storage must separately bind and revalidate the actual file, directory,
/// library generation and retained custody before using this data for effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamDigest {
    checksum: Checksum,
    bytes: u64,
}
impl StreamDigest {
    pub const fn checksum(self) -> Checksum {
        self.checksum
    }
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

/// Hash through EOF with one fixed-size buffer and an explicit size ceiling.
/// Read errors, cancellation and overflow return no partial checksum. A read
/// may consume one sentinel byte beyond the limit to detect overflow; this is
/// not a decoder allocation/work budget or ownership/containment proof.
///
/// This synchronous primitive cannot interrupt an arbitrary blocking Read.
/// Its caller owns bounded execution and must not infer stopped work merely
/// from cancellation requested elsewhere. Actual read errors retain their
/// causes; an executor may independently observe cancellation without erasing
/// those facts. The reader remains borrowed, with no close/cleanup on failure.
pub fn hash_stream(
    reader: &mut impl Read,
    limit: u64,
    cancellation: &WorkCancellation,
) -> Result<StreamDigest> {
    let mut buffer = [0u8; HASH_BUFFER_BYTES];
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    loop {
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let available = limit
            .checked_sub(bytes)
            .ok_or(StorageError::HashSizeLimit)?;
        let request = available.saturating_add(1).min(HASH_BUFFER_BYTES as u64) as usize;
        let count = match reader.read(&mut buffer[..request]) {
            Ok(count) if count <= request => count,
            Ok(_) => {
                return Err(StorageError::io(
                    "hash stream",
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "reader returned more bytes than supplied buffer",
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(StorageError::io("hash stream", error)),
        };
        if count == 0 {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            return Ok(StreamDigest {
                checksum: Checksum::from_bytes(hash.finalize().into()),
                bytes,
            });
        }
        bytes = bytes
            .checked_add(count as u64)
            .ok_or(StorageError::HashSizeLimit)?;
        if bytes > limit {
            return Err(StorageError::HashSizeLimit);
        }
        hash.update(&buffer[..count]);
    }
}
