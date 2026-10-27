//! Read the OS offset for each actual instant on a blocking controller. This
//! never changes the process timezone or pretends that UTC is local time.
pub use time::OffsetDateTime as LocalDateTime;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
fn instant(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok().or_else(|| {
        let bytes = value.as_bytes();
        if bytes.len() == 19 && bytes[10] == b' ' {
            let mut normalized = value.to_owned();
            normalized.replace_range(10..11, "T");
            normalized.push('Z');
            OffsetDateTime::parse(&normalized, &Rfc3339).ok()
        } else {
            None
        }
    })
}
pub fn local(value: &str) -> Option<LocalDateTime> {
    let instant = instant(value)?;
    let offset = UtcOffset::local_offset_at(instant).ok()?;
    Some(instant.to_offset(offset))
}
pub fn utc_now() -> Option<String> {
    OffsetDateTime::now_utc().format(&Rfc3339).ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sqlite_and_provider_instants_keep_their_identity_in_observed_offsets()
    -> Result<(), Box<dyn std::error::Error>> {
        let sql = instant("2026-10-27 09:12:30").ok_or("SQL UTC timestamp")?;
        let provider = instant("2026-10-27T11:12:30+02:00").ok_or("provider timestamp")?;
        assert_eq!(sql.unix_timestamp(), provider.unix_timestamp());
        let offset = UtcOffset::from_hms(1, 0, 0)?;
        let observed = sql.to_offset(offset);
        assert_eq!(observed.hour(), 10);
        assert_eq!(observed.unix_timestamp(), sql.unix_timestamp());
        assert!(instant("not a date").is_none());
        assert!(local("2026-10-27 09:12:30").is_some());
        Ok(())
    }
}
