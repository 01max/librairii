//! Read-only foreign profile evidence and identity-verified rebinding DATA.
//!
//! These types cannot open a library, publish settings or replay journals. Raw
//! history/destination strings remain historical data, never cleanup authority.
//! Filename/collation/filesystem and recovery-path qualification remain distinct.
use crate::{
    Result, StorageError,
    profile::{MAX_PROFILE_BYTES, Profile},
};

/// Origin syntax is explicit; a host must not reinterpret a foreign absolute
/// path as a relative local filename or discover a profile from it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfilePathSyntax {
    Posix,
    Windows,
}

#[derive(Clone, Debug)]
pub struct ForeignProfile {
    original: Vec<u8>,
    profile: Profile,
    syntax: ProfilePathSyntax,
}
impl ForeignProfile {
    /// Decode the strict bounded v1 wire format without resolving or opening
    /// any root. Windows drive/UNC recognition is ONLY an absolute-path syntax
    /// check, not validation of device/reserved/long names or FS capabilities.
    pub fn decode(bytes: &[u8], syntax: ProfilePathSyntax) -> Result<Self> {
        if bytes.len() > MAX_PROFILE_BYTES {
            return Err(StorageError::InvalidProfile);
        }
        let profile: Profile =
            serde_json::from_slice(bytes).map_err(|_| StorageError::InvalidProfile)?;
        if profile.version != 1
            || !matches!(profile.language.as_str(), "en" | "fr")
            || ((profile.complete || profile.recovering)
                && (profile.library_id.is_empty() || !absolute(&profile.root, syntax)))
        {
            return Err(StorageError::InvalidProfile);
        }
        Ok(Self {
            original: bytes.to_vec(),
            profile,
            syntax,
        })
    }
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }
    pub fn profile(&self) -> &Profile {
        &self.profile
    }
    pub const fn syntax(&self) -> ProfilePathSyntax {
        self.syntax
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn prepare_rebinding<'copy>(
        &'copy self,
        copy: &'copy crate::adoption::IndependentCopy,
        cancellation: &crate::database::WorkCancellation,
    ) -> Result<PreparedProfileRebinding<'copy>> {
        if !self.profile.complete && !self.profile.recovering {
            return Err(StorageError::InvalidProfile);
        }
        copy.revalidate(cancellation)?;
        copy.identity().require_spelling(&self.profile.library_id)?;
        let mut profile = self.profile.clone();
        // A v1 JSON profile cannot represent arbitrary native byte paths. Never
        // convert them lossily or rename copied files to make them fit.
        profile.root = copy
            .root()
            .to_str()
            .ok_or(StorageError::InvalidProfile)?
            .to_owned();
        let rebound = crate::profile::encoded_document(&profile)?;
        Ok(PreparedProfileRebinding {
            source: self,
            copy,
            rebound,
        })
    }
}

fn absolute(root: &str, syntax: ProfilePathSyntax) -> bool {
    if root.contains('\0') {
        return false;
    }
    match syntax {
        ProfilePathSyntax::Posix => root.starts_with('/'),
        ProfilePathSyntax::Windows => {
            let bytes = root.as_bytes();
            if bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/')
            {
                return true;
            }
            // Ordinary UNC server/share only. Extended/device namespaces need
            // independent target qualification; do not guess or strip prefixes.
            if let Some(tail) = root.strip_prefix("\\\\") {
                let mut parts = tail.split('\\');
                return matches!(parts.next(), Some(server) if !server.is_empty() && !matches!(server, "." | ".." | "?"))
                    && matches!(parts.next(), Some(share) if !share.is_empty() && !matches!(share, "." | ".."));
            }
            false
        }
    }
}

/// A proposed native-host profile DOCUMENT borrowing the actual independent
/// copy/lease. It has no publish/runtime/cleanup method. A future activation
/// must separately establish the native namespace and rebind/refuse recovery.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[derive(Debug)]
pub struct PreparedProfileRebinding<'copy> {
    source: &'copy ForeignProfile,
    copy: &'copy crate::adoption::IndependentCopy,
    rebound: crate::profile::ProfileDocument,
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
impl PreparedProfileRebinding<'_> {
    pub fn original(&self) -> &ForeignProfile {
        self.source
    }
    pub fn proposed_document(&self) -> &crate::profile::ProfileDocument {
        &self.rebound
    }
    pub fn revalidate(&self, cancellation: &crate::database::WorkCancellation) -> Result<()> {
        self.copy.revalidate(cancellation)?;
        self.copy
            .identity()
            .require_spelling(&self.source.profile.library_id)
    }
}
