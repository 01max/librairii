//! Frozen Go time.Parse(RFC3339) acceptance and UTC RFC3339Nano formatting.
use super::*;
fn invalid() -> MetadataError {
    MetadataError::InvalidCatalog
}
fn number(s: &str) -> Result<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    s.parse().map_err(|_| invalid())
}
pub(super) fn utc_time(s: &str) -> Result<String> {
    if !s.is_ascii()
        || s.len() < 19
        || s.get(4..5) != Some("-")
        || s.get(7..8) != Some("-")
        || s.get(10..11) != Some("T")
    {
        return Err(invalid());
    }
    let year = number(&s[..4])? as i32;
    let month = number(&s[5..7])? as u8;
    let day = number(&s[8..10])? as u8;
    let rest = &s[11..];
    let colon = rest.find(':').ok_or_else(invalid)?;
    if !(1..=2).contains(&colon) {
        return Err(invalid());
    }
    let hour = number(&rest[..colon])? as u8;
    let rest = &rest[colon + 1..];
    if rest.len() < 6 || rest.get(2..3) != Some(":") {
        return Err(invalid());
    }
    let minute = number(&rest[..2])? as u8;
    let second = number(&rest[3..5])? as u8;
    let mut rest = &rest[5..];
    let mut nanos = 0;
    if rest.starts_with(['.', ',']) {
        rest = &rest[1..];
        let count = rest.bytes().take_while(u8::is_ascii_digit).count();
        if count == 0 {
            return Err(invalid());
        }
        let digits = &rest[..count.min(9)];
        nanos = number(digits)? * 10u32.pow(9 - digits.len() as u32);
        rest = &rest[count..];
    }
    let offset = if rest == "Z" {
        0
    } else {
        if rest.len() != 6 || !rest.starts_with(['+', '-']) || &rest[3..4] != ":" {
            return Err(invalid());
        }
        let h = number(&rest[1..3])?;
        let m = number(&rest[4..6])?;
        if h > 24 || m > 60 {
            return Err(invalid());
        }
        let seconds = (h * 3600 + m * 60) as i64;
        if rest.starts_with('-') {
            -seconds
        } else {
            seconds
        }
    };
    let date = time::Date::from_calendar_date(
        year,
        time::Month::try_from(month).map_err(|_| invalid())?,
        day,
    )
    .map_err(|_| invalid())?;
    let clock = time::Time::from_hms_nano(hour, minute, second, nanos).map_err(|_| invalid())?;
    let utc = time::PrimitiveDateTime::new(date, clock)
        .checked_sub(time::Duration::seconds(offset))
        .ok_or_else(invalid)?;
    let year = if utc.year() < 0 {
        format!("-{:04}", -utc.year())
    } else {
        format!("{:04}", utc.year())
    };
    let fraction = if utc.nanosecond() == 0 {
        String::new()
    } else {
        format!(".{:09}", utc.nanosecond())
            .trim_end_matches('0')
            .to_owned()
    };
    Ok(format!(
        "{year}-{:02}-{:02}T{:02}:{:02}:{:02}{fraction}Z",
        u8::from(utc.month()),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second()
    ))
}
