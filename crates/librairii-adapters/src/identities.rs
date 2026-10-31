//! Fresh native operation identities from the host's random source.
use std::io::Read;
pub fn job_id() -> std::io::Result<librairii_core::JobId> {
    let mut bytes = [0_u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(librairii_core::JobId::from_bytes(bytes))
}
