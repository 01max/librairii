//! Minimal bounded encoded-header method preflight. The candidate remains the
//! authoritative container parser; this only preserves Go's codec set before a
//! header decoder can be instantiated. No payload is decoded or extracted.
use super::*;
struct Header<'a>(&'a [u8]);
impl<'a> Header<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let (head, tail) = self.0.split_at_checked(count).ok_or_else(invalid)?;
        self.0 = tail;
        Ok(head)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn number(&mut self) -> Result<u64> {
        let first = self.byte()?;
        let mut value = 0u64;
        for n in 0..8 {
            let mask = 0x80u8 >> n;
            if first & mask == 0 {
                return Ok(value | (u64::from(first & (mask - 1)) << (8 * n)));
            }
            value |= u64::from(self.byte()?) << (8 * n);
        }
        Ok(value)
    }
    fn count(&mut self, ceiling: usize) -> Result<usize> {
        let n = usize::try_from(self.number()?).map_err(|_| invalid())?;
        if n > ceiling {
            return Err(invalid());
        }
        Ok(n)
    }
    fn expect(&mut self, byte: u8) -> Result<()> {
        if self.byte()? != byte {
            return Err(invalid());
        }
        Ok(())
    }
}
pub(super) fn baseline_method(id: &[u8]) -> bool {
    !matches!(
        id,
        [0x03, 0x03, 0x04, 0x01] | [0x03, 0x03, 0x07, 0x01] | [0x0b]
    )
}
pub(super) fn check<R: Read + Seek>(
    source: &mut Checked<R>,
    size: u64,
    limits: DecoderLimits,
) -> Result<()> {
    source.seek(SeekFrom::Start(0)).map_err(|_| invalid())?;
    let mut start = [0u8; 32];
    source.read_exact(&mut start).map_err(|_| invalid())?;
    if &start[..6] != b"7z\xbc\xaf\x27\x1c" {
        return Err(invalid());
    }
    let offset = u64::from_le_bytes(start[12..20].try_into().map_err(|_| invalid())?);
    let count = u64::from_le_bytes(start[20..28].try_into().map_err(|_| invalid())?);
    let position = 32u64.checked_add(offset).ok_or_else(invalid)?;
    if count > limits.header_bytes || position.checked_add(count).is_none_or(|end| end > size) {
        return Err(invalid());
    }
    source
        .seek(SeekFrom::Start(position))
        .map_err(|_| invalid())?;
    let count = usize::try_from(count).map_err(|_| invalid())?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(count).map_err(|_| invalid())?;
    bytes.resize(count, 0);
    source.read_exact(&mut bytes).map_err(|_| invalid())?;
    source.seek(SeekFrom::Start(0)).map_err(|_| invalid())?;
    let mut h = Header(&bytes);
    if h.byte()? != 0x17 {
        return Ok(());
    }
    h.expect(0x06)?;
    let _pack_position = h.number()?;
    let packs = h.count(10_000)?;
    loop {
        match h.byte()? {
            0x00 => break,
            0x09 => {
                for _ in 0..packs {
                    h.number()?;
                }
            }
            0x0a => {
                let defined = if h.byte()? != 0 {
                    packs
                } else {
                    let flags = h.take(packs.div_ceil(8))?;
                    (0..packs)
                        .filter(|i| flags[i / 8] & (0x80 >> (i % 8)) != 0)
                        .count()
                };
                h.take(defined.checked_mul(4).ok_or_else(invalid)?)?;
            }
            _ => return Err(invalid()),
        }
    }
    h.expect(0x07)?;
    h.expect(0x0b)?;
    // Frozen Go allows exactly one encoded-header folder.
    if h.number()? != 1 {
        return Err(invalid());
    }
    h.expect(0)?;
    let coders = h.count(8)?;
    for _ in 0..coders {
        let flags = h.byte()?;
        if flags & 0xc0 != 0 {
            return Err(invalid());
        }
        let id = h.take(usize::from(flags & 15))?;
        if !baseline_method(id) {
            return Err(invalid());
        }
        if flags & 0x10 != 0 {
            h.count(8)?;
            h.count(8)?;
        }
        if flags & 0x20 != 0 {
            let n = h.count(bytes.len())?;
            h.take(n)?;
        }
    }
    Ok(())
}
