//! External/native adapters behind typed application and domain boundaries.
#![forbid(unsafe_code)]
pub mod archives;
pub mod lunii;

pub mod thumbnails;

pub mod datetime;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod identities;
