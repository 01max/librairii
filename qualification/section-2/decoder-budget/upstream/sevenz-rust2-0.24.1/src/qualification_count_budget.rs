//! QUAL2.10-392-COUNT-BUDGET: targeted experimental guards, NOT a RAM bound.
//! Header allocation and every other decoder allocation/work path remain open.
use crate::Error;

/// Experimental file-vector count ceiling; not product qualification.
pub const MAX_FILES: u64 = 10_000;
/// Experimental per-block coder count ceiling; not accepted-codec policy.
pub const MAX_CODERS: u64 = 64;
/// Per-target-vector requested bytes only; no total memory guarantee.
pub const MAX_VECTOR_BYTES: usize = 8 << 20;

/// Checks all arithmetic before either targeted vector allocation. This function
/// allocates nothing, including on refusal. Header-byte validity remains separate.
pub fn checked_count<T>(
    count: u64,
    header_limit: usize,
    field: &'static str,
    max_count: u64,
) -> Result<usize, Error> {
    let converted = usize::try_from(count).ok();
    let requested_bytes = converted.and_then(|n| n.checked_mul(size_of::<T>()));
    let reason = if converted.is_none() {
        Some("count_not_representable")
    } else if requested_bytes.is_none() {
        Some("size_overflow")
    } else if count > max_count {
        Some("count_ceiling")
    } else if requested_bytes.is_some_and(|bytes| bytes > MAX_VECTOR_BYTES) {
        Some("vector_byte_ceiling")
    } else {
        None
    };
    if let Some(reason) = reason {
        return Err(Error::QualificationCountBudget {
            field,
            count,
            max_count,
            element_bytes: size_of::<T>(),
            requested_bytes,
            max_bytes: MAX_VECTOR_BYTES,
            reason,
        });
    }
    // The checked conversion above is also mandatory on 32-bit hosts.
    let n = converted.expect("converted count validated above");
    if n > header_limit {
        return Err(Error::other("count exceeds header-byte bound"));
    }
    Ok(n)
}
