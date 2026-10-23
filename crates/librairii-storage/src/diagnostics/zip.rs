//! Bounded stored ZIP, six fixed entries at most. No reader, compression,
//! encryption or ZIP64. PKWARE APPNOTE 6.3.10 sections 4.3.7, 4.3.12, 4.3.16:
//! https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
use super::*;
fn word(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_le_bytes());
}
fn dword(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_le_bytes());
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
        }
    }
    !crc
}
pub(super) fn encode(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    if entries.is_empty() || entries.len() > MAX_LOG_FILES + 1 {
        return Err(StorageError::HashSizeLimit);
    }
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, bytes) in entries {
        if bytes.len() > MAX_LOG_BYTES
            || name.len() > 64
            || !(name == "diagnostics.json"
                || (0..MAX_LOG_FILES).any(|i| {
                    name == &if i == 0 {
                        "logs/events.jsonl".into()
                    } else {
                        format!("logs/events.{i}.jsonl")
                    }
                }))
        {
            return Err(StorageError::HashSizeLimit);
        }
        let offset = u32::try_from(out.len()).map_err(|_| StorageError::HashSizeLimit)?;
        let size = u32::try_from(bytes.len()).map_err(|_| StorageError::HashSizeLimit)?;
        let length = u16::try_from(name.len()).map_err(|_| StorageError::HashSizeLimit)?;
        let crc = crc32(bytes);
        dword(&mut out, 0x04034b50);
        word(&mut out, 20);
        word(&mut out, 0x800);
        word(&mut out, 0);
        word(&mut out, 0);
        word(&mut out, 0x21);
        dword(&mut out, crc);
        dword(&mut out, size);
        dword(&mut out, size);
        word(&mut out, length);
        word(&mut out, 0);
        out.extend(name.as_bytes());
        out.extend(bytes);
        dword(&mut directory, 0x02014b50);
        word(&mut directory, 0x0314);
        word(&mut directory, 20);
        word(&mut directory, 0x800);
        word(&mut directory, 0);
        word(&mut directory, 0);
        word(&mut directory, 0x21);
        dword(&mut directory, crc);
        dword(&mut directory, size);
        dword(&mut directory, size);
        word(&mut directory, length);
        word(&mut directory, 0);
        word(&mut directory, 0);
        word(&mut directory, 0);
        word(&mut directory, 0);
        dword(&mut directory, 0o100600 << 16);
        dword(&mut directory, offset);
        directory.extend(name.as_bytes());
    }
    let offset = u32::try_from(out.len()).map_err(|_| StorageError::HashSizeLimit)?;
    let size = u32::try_from(directory.len()).map_err(|_| StorageError::HashSizeLimit)?;
    let count = u16::try_from(entries.len()).map_err(|_| StorageError::HashSizeLimit)?;
    out.extend(directory);
    dword(&mut out, 0x06054b50);
    word(&mut out, 0);
    word(&mut out, 0);
    word(&mut out, count);
    word(&mut out, count);
    dword(&mut out, size);
    dword(&mut out, offset);
    word(&mut out, 0);
    Ok(out)
}
