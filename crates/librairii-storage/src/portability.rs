//! Read-only foreign profile evidence and identity-verified rebinding DATA.
//!
//! These types cannot open a library, publish settings or replay journals. Raw
//! history/destination strings remain historical data, never cleanup authority.
//! Filename diagnostics never establish collation/filesystem capabilities or
//! recovery-path authority. Passing a lexical check is not adoption permission.
use crate::{
    Result, StorageError,
    adoption::ClosedManifest,
    profile::{MAX_PROFILE_BYTES, Profile},
};

/// Documented Win32 naming issues, not a complete target-filesystem predicate.
/// More than one issue can apply to the same original component.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowsNameProblems {
    pub reserved_character: bool,
    pub control_character: bool,
    pub device_name: bool,
    pub trailing_space_or_dot: bool,
}

/// Borrows exact declared spelling; never a renamed path or custody witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsNameIssue<'manifest> {
    pub raw_relative_path: &'manifest str,
    pub raw_component: &'manifest str,
    pub component_index: usize,
    pub problems: WindowsNameProblems,
}

/// Stream documented Windows name problems from the bounded declared manifest.
///
/// No path is opened, normalized, casefolded, shortened, rebound or deleted.
/// The iterator borrows the manifest and allocates no collected issue list;
/// callers retain their own bounded reporting policy. Directory and file
/// components receive the same checks. Empty output DOES NOT establish safe
/// adoption: actual case/Unicode equivalence, aliases/reparse points, component
/// and total path limits, long-path APIs, network/removable/offline/read-only
/// storage, locking and durability still require target capability proof.
/// A declared manifest itself does not prove any source object exists.
///
/// Rules: Microsoft "Naming Files, Paths, and Namespaces" naming conventions.
/// https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
pub fn windows_name_issues(
    manifest: &ClosedManifest,
) -> impl Iterator<Item = WindowsNameIssue<'_>> {
    manifest.relative_paths().flat_map(|raw_relative_path| {
        raw_relative_path.split('/').enumerate().filter_map(
            move |(component_index, raw_component)| {
                let problems = WindowsNameProblems {
                    reserved_character: raw_component
                        .contains(['<', '>', ':', '"', '\\', '|', '?', '*']),
                    control_character: raw_component.chars().any(|c| c <= '\u{1f}'),
                    device_name: windows_device_name(raw_component),
                    trailing_space_or_dot: raw_component.ends_with([' ', '.']),
                };
                (problems != WindowsNameProblems::default()).then_some(WindowsNameIssue {
                    raw_relative_path,
                    raw_component,
                    component_index,
                    problems,
                })
            },
        )
    })
}

fn windows_device_name(component: &str) -> bool {
    // ASCII comparison here recognizes the documented device tokens only. It
    // does not approximate Windows filename collation or Unicode case folding.
    let stem = component.split('.').next().unwrap_or_default();
    if ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return true;
    }
    let Some(prefix) = stem.get(..3) else {
        return false;
    };
    if !prefix.eq_ignore_ascii_case("COM") && !prefix.eq_ignore_ascii_case("LPT") {
        return false;
    }
    // Prefix is now exactly three ASCII bytes, so this boundary is proven.
    let mut suffix = stem[3..].chars();
    matches!(suffix.next(), Some('1'..='9' | '¹' | '²' | '³')) && suffix.next().is_none()
}

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
