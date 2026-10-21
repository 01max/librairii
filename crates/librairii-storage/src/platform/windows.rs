//! Private Windows handle observations. No path/data constructor here grants
//! mutation, no-clobber, configuration replacement or directory durability.
use crate::{Result, StorageError};
use std::{
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::{self, Read},
    mem::{align_of, offset_of, size_of},
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_BASIC_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_128, FILE_ID_INFO,
    FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FILE_STANDARD_INFO, FileAttributeTagInfo, FileBasicInfo, FileIdInfo, FileStandardInfo,
    GetFileInformationByHandleEx, SYNCHRONIZE,
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
            FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
        },
    },
    Win32::{
        Foundation::{INVALID_HANDLE_VALUE, OBJ_DONT_REPARSE, UNICODE_STRING},
        System::IO::{IO_STATUS_BLOCK, IO_STATUS_BLOCK_0},
    },
};

/// Read-only child object retaining the real parent library/OS lease. This is
/// not a stage/publication/cleanup capability or Windows native activation.
#[derive(Debug)]
pub(crate) struct ReadChild<'lease> {
    file: File,
    identity: FileIdentity,
    _lease: &'lease super::lock::LibraryLease,
}
// Drop checking retains the library borrow through the actual File close.
impl Drop for ReadChild<'_> {
    fn drop(&mut self) {}
}
impl ReadChild<'_> {
    pub(crate) fn identity(&self) -> FileIdentity {
        self.identity
    }
}
impl Read for ReadChild<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

fn checked_child_name(name: &OsStr) -> Result<Vec<u16>> {
    // UNICODE_STRING lengths are BYTES, not characters; cap allocation before
    // collecting arbitrary caller input. EncodeWide preserves native surrogates.
    let value: Vec<_> = name
        .encode_wide()
        .take((u16::MAX as usize / 2) + 1)
        .collect();
    if value.is_empty()
        || value == [46]
        || value == [46, 46]
        || value.len() > u16::MAX as usize / 2
        || value.iter().any(|c| matches!(*c, 0 | 47 | 58 | 92))
    {
        return Err(StorageError::InvalidManagedPath);
    }
    Ok(value)
}

/// Open ONE existing regular child relative to the retained library root.
/// FILE_OPEN is deliberately separate from any future create-new operation.
///
/// # Invariants
/// The retained root is obtained only from the actual sealed library lease;
/// no arbitrary path/ID creates a mutable capability. All inputs/outputs are
/// initialized, exactly aligned SDK layouts. Name units/checked BYTE lengths
/// and OBJECT_ATTRIBUTES pointers live throughout synchronous completion.
/// NONALERT plus SYNCHRONIZE fixes synchronous I/O; NtCreateFile documents final
/// IO_STATUS_BLOCK completion on return. No event/APC/overlapped/pending storage
/// is supplied or allowed to escape. NTSTATUS is not BOOL/GetLastError.
/// Successful unique ownership transfers once into std File; errors never
/// create/overwrite/delete bytes. No directory-create/flush/replace fallback.
/// Sources: pinned NtCreateFile and OBJECT_ATTRIBUTES primary contracts.
#[expect(
    unsafe_code,
    reason = "necessary synchronous root-relative existing-child open; exact local buffers and unique returned-handle ownership documented per call"
)]
pub(crate) fn open_read_child<'lease>(
    lease: &'lease super::lock::LibraryLease,
    name: &OsStr,
) -> Result<ReadChild<'lease>> {
    let mut units = checked_child_name(name)?;
    let (parent, expected_parent) = lease.retained_windows_root();
    let facts = query_identity(parent)?;
    if !facts.directory || facts.identity != expected_parent {
        return Err(StorageError::LibraryLeaseChanged);
    }
    let bytes = u16::try_from(
        units
            .len()
            .checked_mul(2)
            .ok_or(StorageError::InvalidManagedPath)?,
    )
    .map_err(|_| StorageError::InvalidManagedPath)?;
    let text = UNICODE_STRING {
        Length: bytes,
        MaximumLength: bytes,
        Buffer: units.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| StorageError::CompatibilityInvariant)?,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &text,
        Attributes: OBJ_DONT_REPARSE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut completion = IO_STATUS_BLOCK {
        // Initialize the full pointer-width union storage, including the bytes
        // above its narrower NTSTATUS member on 64-bit Windows. No union read.
        Anonymous: IO_STATUS_BLOCK_0 {
            Pointer: std::ptr::null_mut(),
        },
        Information: 0,
    };
    let mut handle = std::ptr::null_mut();
    // SAFETY: parent borrow keeps its verified directory HANDLE alive. Name
    // and every initialized aligned repr(C) local above remain stable and
    // unaliased during this synchronous, final-completion call. Checked lengths
    // include no terminator; optional buffers are NULL with zero lengths. Fixed
    // FILE_OPEN cannot create/supersede and NON_DIRECTORY/DONT_REPARSE constrain
    // lookup. No pointer escapes; no pending/event/APC/async mode is requested.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &attributes,
            &mut completion,
            std::ptr::null(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(StorageError::io(
            "open retained Windows child",
            io::Error::other(format!("NTSTATUS 0x{:08X}", status as u32)),
        ));
    }
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(StorageError::CompatibilityInvariant);
    }
    // SAFETY: successful NtCreateFile transfers one valid owned file HANDLE
    // into this sole RAII File. There is no other Rust owner, close, duplicate
    // or outstanding native I/O; the handle stays live through every query/read
    // and closes once, before the retained lease borrow may end.
    let file = unsafe { File::from_raw_handle(handle) };
    let child = query_identity(&file)?;
    if child.directory || child.links != 1 {
        return Err(StorageError::PathEscape);
    }
    Ok(ReadChild {
        file,
        identity: child.identity,
        _lease: lease,
    })
}

/// Scoped to the retained live handles, not a permanent identifier after delete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    volume: u64,
    id: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObjectFacts {
    pub(crate) identity: FileIdentity,
    pub(crate) directory: bool,
    pub(crate) links: u32,
}

// The SDK structure uses Rust bool fields. Receive the native BOOLEAN bytes
// into u8 instead, so any nonzero native true value has valid Rust storage.
// Layout is checked against the exact pinned binding on the actual target.
#[repr(C)]
struct StandardBytes {
    allocation: i64,
    end: i64,
    links: u32,
    delete_pending: u8,
    directory: u8,
}
const _: () = {
    assert!(size_of::<StandardBytes>() == size_of::<FILE_STANDARD_INFO>());
    assert!(align_of::<StandardBytes>() == align_of::<FILE_STANDARD_INFO>());
    assert!(offset_of!(StandardBytes, links) == offset_of!(FILE_STANDARD_INFO, NumberOfLinks));
    assert!(
        offset_of!(StandardBytes, delete_pending) == offset_of!(FILE_STANDARD_INFO, DeletePending)
    );
    assert!(offset_of!(StandardBytes, directory) == offset_of!(FILE_STANDARD_INFO, Directory));
};

/// Borrow a live std-owned file handle for synchronous observations only.
///
/// # Invariants
/// No handle is closed/transferred/duplicated by this function. The shared File
/// borrow keeps it live throughout all synchronous calls. Local initialized
/// repr(C) buffers have exact matching SDK layouts/classes, checked byte sizes,
/// alignment and unique mutable access; no pointer escapes or pending operation
/// exists. Calls use BOOL/last_os_error, not NTSTATUS. Returned facts do not prove
/// coherent file CONTENT, pathname ownership or absence of a later mutation.
///
/// Source: Microsoft GetFileInformationByHandleEx and FILE_ID_INFO contracts.
/// std metadata has no equivalent volume+128-bit identity observation.
#[expect(
    unsafe_code,
    reason = "necessary exact-size Windows handle information queries; buffers and borrowed handle invariants documented per call"
)]
pub(crate) fn query_identity(file: &File) -> Result<ObjectFacts> {
    // A safe preliminary disk-file/directory observation rejects handles such
    // as pipes before invoking this API (which is not a pipe-information API).
    let metadata = file
        .metadata()
        .map_err(|e| StorageError::io("inspect Windows object kind", e))?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(StorageError::PathEscape);
    }
    let handle = file.as_raw_handle();
    let mut tag = FILE_ATTRIBUTE_TAG_INFO {
        FileAttributes: 0,
        ReparseTag: 0,
    };
    let mut id = FILE_ID_INFO {
        VolumeSerialNumber: 0,
        FileId: FILE_ID_128 {
            Identifier: [0; 16],
        },
    };
    let mut basic = FILE_BASIC_INFO {
        CreationTime: 0,
        LastAccessTime: 0,
        LastWriteTime: 0,
        ChangeTime: 0,
        FileAttributes: 0,
    };
    let mut standard = StandardBytes {
        allocation: 0,
        end: 0,
        links: 0,
        delete_pending: 0,
        directory: 0,
    };
    let size = |bytes| u32::try_from(bytes).map_err(|_| StorageError::CompatibilityInvariant);
    let tag_size = size(size_of::<FILE_ATTRIBUTE_TAG_INFO>())?;
    let id_size = size(size_of::<FILE_ID_INFO>())?;
    let basic_size = size(size_of::<FILE_BASIC_INFO>())?;
    let standard_size = size(size_of::<StandardBytes>())?;
    // SAFETY: live borrowed non-pipe File; &mut tag is aligned initialized
    // FILE_ATTRIBUTE_TAG_INFO, class and checked byte size match exactly;
    // Windows writes only synchronously, with no concurrent Rust alias.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileAttributeTagInfo,
            (&mut tag as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            tag_size,
        )
    } == 0
    {
        return Err(StorageError::io(
            "query Windows reparse attributes",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: same live borrowed handle; unique initialized FILE_ID_INFO buffer
    // matches FileIdInfo and checked size; pointer remains local until return.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&mut id as *mut FILE_ID_INFO).cast(),
            id_size,
        )
    } == 0
    {
        return Err(StorageError::io(
            "query Windows volume and 128-bit identity",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: same synchronous handle query, exact FileBasicInfo class with a
    // uniquely borrowed initialized/aligned FILE_BASIC_INFO of checked size.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileBasicInfo,
            (&mut basic as *mut FILE_BASIC_INFO).cast(),
            basic_size,
        )
    } == 0
    {
        return Err(StorageError::io(
            "query Windows basic attributes",
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: StandardBytes is initialized repr(C), byte/alignment/field-layout
    // checked against FILE_STANDARD_INFO. Its BOOLEAN receive fields are u8,
    // valid for every written byte. Class/size match; no pointer escapes, no
    // ownership transfer and no Rust alias accesses during this synchronous call.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileStandardInfo,
            (&mut standard as *mut StandardBytes).cast(),
            standard_size,
        )
    } == 0
    {
        return Err(StorageError::io(
            "query Windows type and link count",
            std::io::Error::last_os_error(),
        ));
    }
    if (tag.FileAttributes | basic.FileAttributes) & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || tag.ReparseTag != 0
        || standard.delete_pending != 0
        || standard.end < 0
        || standard.allocation < 0
        || (standard.directory != 0) != metadata.is_dir()
    {
        return Err(StorageError::PathEscape);
    }
    Ok(ObjectFacts {
        identity: FileIdentity {
            volume: id.VolumeSerialNumber,
            id: id.FileId.Identifier,
        },
        directory: standard.directory != 0,
        links: standard.links,
    })
}

/// Readable terminal-object observation and no-delete-sharing pin, NOT a
/// descriptor-relative ancestry/mutation capability. No reparse fallback.
///
/// Attribute-only access does not participate in Windows sharing checks. The
/// data/list access is required to make the retained no-delete share effective;
/// refusal to grant that access is an error, never a metadata-only fallback.
pub(crate) fn metadata_witness(path: &Path, directory: bool) -> Result<(File, FileIdentity)> {
    let read_access = if directory {
        FILE_LIST_DIRECTORY
    } else {
        FILE_READ_DATA
    };
    let file = OpenOptions::new()
        .read(true)
        .access_mode(FILE_READ_ATTRIBUTES | read_access)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|e| StorageError::io("retain Windows metadata witness", e))?;
    let facts = query_identity(&file)?;
    if facts.directory != directory || (!directory && facts.links != 1) {
        return Err(StorageError::PathEscape);
    }
    Ok((file, facts.identity))
}

// Qualification observation only. No caller uses this as a directory mutation,
// flush, custody, restart or physical-storage precondition. The finite native
// tests must establish GetDriveTypeW's volume-GUID applicability on that host.
mod fixed_ntfs {
    use super::*;
    use windows_sys::Win32::{
        Storage::FileSystem::{
            FILE_NAME_OPENED, GetDriveTypeW, GetFinalPathNameByHandleW,
            GetVolumeInformationByHandleW, VOLUME_NAME_GUID,
        },
        System::WindowsProgramming::{DRIVE_FIXED, DRIVE_UNKNOWN},
    };

    const FS_CAPACITY: usize = 16;
    const PATH_CAPACITY: usize = 1024;
    const GUID_ROOT_UNITS: usize = 49;

    trait Calls {
        fn facts(&mut self, file: &File) -> Result<ObjectFacts>;
        fn filesystem(&mut self, file: &File, buffer: &mut [u16]) -> Result<()>;
        fn opened_path(&mut self, file: &File, buffer: &mut [u16]) -> Result<u32>;
        fn drive_type(&mut self, root: &[u16; GUID_ROOT_UNITS + 1]) -> u32;
    }
    struct Native;
    impl Calls for Native {
        fn facts(&mut self, file: &File) -> Result<ObjectFacts> {
            query_identity(file)
        }
        fn filesystem(&mut self, file: &File, buffer: &mut [u16]) -> Result<()> {
            filesystem(file, buffer)
        }
        fn opened_path(&mut self, file: &File, buffer: &mut [u16]) -> Result<u32> {
            opened_path(file, buffer)
        }
        fn drive_type(&mut self, root: &[u16; GUID_ROOT_UNITS + 1]) -> u32 {
            drive_type(root)
        }
    }

    /// Observe only the current OS-reported fixed NTFS classification of this
    /// SAME borrowed handle. None or Err is Unknown/refusal, never a locality
    /// fallback. Calls are not atomic; repeated facts constrain this observation
    /// but cannot establish physical/permanent storage or directory durability.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "released finite Windows qualification helper; not yet a production filesystem precondition"
        )
    )]
    pub(super) fn observe(file: &File) -> Result<Option<FileIdentity>> {
        observe_with(file, &mut Native, FS_CAPACITY, PATH_CAPACITY)
    }

    fn observe_with(
        file: &File,
        calls: &mut impl Calls,
        fs_capacity: usize,
        path_capacity: usize,
    ) -> Result<Option<FileIdentity>> {
        // Fixed initialized receive storage, including the zero terminator.
        // Smaller slices are solely for actual native truncation controls.
        if !(1..=FS_CAPACITY).contains(&fs_capacity)
            || !(1..=PATH_CAPACITY).contains(&path_capacity)
        {
            return Ok(None);
        }
        let mut fs = [0u16; FS_CAPACITY];
        let mut path = [0u16; PATH_CAPACITY];
        let before = calls.facts(file)?;
        calls.filesystem(file, &mut fs[..fs_capacity])?;
        if !exact_ntfs(&fs[..fs_capacity]) {
            return Ok(None);
        }
        let length = calls.opened_path(file, &mut path[..path_capacity])?;
        let Some(root) = guid_root(&path[..path_capacity], length) else {
            return Ok(None);
        };
        if calls.drive_type(&root) != DRIVE_FIXED {
            return Ok(None);
        }
        // The volume APIs and drive-type call do not form an atomic snapshot.
        // Re-observe filesystem, derived root and object facts around them.
        fs.fill(0);
        calls.filesystem(file, &mut fs[..fs_capacity])?;
        path.fill(0);
        let length = calls.opened_path(file, &mut path[..path_capacity])?;
        let after = calls.facts(file)?;
        if !exact_ntfs(&fs[..fs_capacity])
            || guid_root(&path[..path_capacity], length) != Some(root)
            || before.identity != after.identity
            || before.directory != after.directory
            || before.links != after.links
        {
            return Ok(None);
        }
        Ok(Some(before.identity))
    }

    fn exact_ntfs(buffer: &[u16]) -> bool {
        buffer.iter().position(|unit| *unit == 0) == Some(4)
            && buffer.starts_with(&[78, 84, 70, 83])
    }

    fn guid_root(buffer: &[u16], length: u32) -> Option<[u16; GUID_ROOT_UNITS + 1]> {
        let length = usize::try_from(length).ok()?;
        // On insufficient capacity, the API may return required capacity
        // including NUL. Such a length is never a successfully received path.
        if length < GUID_ROOT_UNITS
            || length >= buffer.len()
            || buffer[length] != 0
            || buffer[..length].contains(&0)
        {
            return None;
        }
        let prefix = [92, 92, 63, 92, 86, 111, 108, 117, 109, 101, 123];
        if buffer[..11] != prefix || buffer[47] != 125 || buffer[48] != 92 {
            return None;
        }
        for (index, unit) in buffer[11..47].iter().enumerate() {
            if matches!(index, 8 | 13 | 18 | 23) {
                if *unit != 45 {
                    return None;
                }
            } else if !matches!(*unit, 48..=57 | 65..=70 | 97..=102) {
                return None;
            }
        }
        let mut root = [0u16; GUID_ROOT_UNITS + 1];
        root[..GUID_ROOT_UNITS].copy_from_slice(&buffer[..GUID_ROOT_UNITS]);
        Some(root)
    }

    /// # Invariants
    /// The borrowed std File owns this live HANDLE through the synchronous call.
    /// The uniquely borrowed initialized bounded WCHAR output is valid for its
    /// checked length; all unused optional output pointers are NULL/size0.
    /// Nothing is transferred, closed, mutated or retained by Windows.
    #[expect(
        unsafe_code,
        reason = "necessary readonly volume-name observation on the retained handle; bounded initialized output and NULL optional fields"
    )]
    fn filesystem(file: &File, buffer: &mut [u16]) -> Result<()> {
        let capacity =
            u32::try_from(buffer.len()).map_err(|_| StorageError::CompatibilityInvariant)?;
        // SAFETY: same live borrowed File; initialized/aligned u16 slice is
        // uniquely mutable and valid for exactly capacity units, no alias or
        // pointer escapes; every unused optional buffer/output is NULL/zero.
        let status = unsafe {
            GetVolumeInformationByHandleW(
                file.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                buffer.as_mut_ptr(),
                capacity,
            )
        };
        if status == 0 {
            return Err(StorageError::io(
                "observe retained Windows filesystem",
                io::Error::last_os_error(),
            ));
        }
        Ok(())
    }

    /// # Invariants
    /// The live shared File borrow and uniquely mutable fixed WCHAR output
    /// survive this synchronous call. Flags request opened name + volume GUID,
    /// with no pathname input, ownership transfer or retained pointer. Returned
    /// length is checked separately before reading or deriving any root.
    #[expect(
        unsafe_code,
        reason = "necessary readonly opened volume-GUID path observation on the same retained handle"
    )]
    fn opened_path(file: &File, buffer: &mut [u16]) -> Result<u32> {
        let capacity =
            u32::try_from(buffer.len()).map_err(|_| StorageError::CompatibilityInvariant)?;
        // SAFETY: same std-owned live handle; initialized u16 output is aligned,
        // valid for exactly capacity and unaliased for this synchronous write;
        // no pointers escape, no allocation or async operation is delegated.
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buffer.as_mut_ptr(),
                capacity,
                FILE_NAME_OPENED | VOLUME_NAME_GUID,
            )
        };
        if length == 0 {
            return Err(StorageError::io(
                "observe retained Windows volume GUID path",
                io::Error::last_os_error(),
            ));
        }
        Ok(length)
    }

    /// # Invariants
    /// The caller supplies only the fixed, fully validated GUID root produced
    /// above, including its trailing backslash and one NUL. The borrowed slice
    /// stays initialized/live throughout this synchronous readonly call.
    #[expect(
        unsafe_code,
        reason = "necessary current OS drive-class observation of only the validated handle-derived volume GUID root"
    )]
    fn drive_type(root: &[u16; GUID_ROOT_UNITS + 1]) -> u32 {
        if guid_root(root, GUID_ROOT_UNITS as u32) != Some(*root) {
            return DRIVE_UNKNOWN;
        }
        // SAFETY: the complete fixed GUID root including NUL was validated
        // above. The immutable u16 buffer stays live and unaliased
        // throughout the synchronous call; no pointer is retained or mutated.
        unsafe { GetDriveTypeW(root.as_ptr()) }
    }

    #[cfg(test)]
    mod qualification {
        use super::*;
        use librairii_testkit::TestRoots;
        use std::{error::Error, fs};
        use windows_sys::Win32::System::WindowsProgramming::{
            DRIVE_NO_ROOT_DIR, DRIVE_REMOTE, DRIVE_REMOVABLE,
        };
        type TestResult = std::result::Result<(), Box<dyn Error>>;

        #[test]
        fn windows_fixed_ntfs_positive_uses_same_retained_handle_and_complete_guid_root()
        -> TestResult {
            let roots = TestRoots::new()?;
            let path = roots.library().join("Éte # %2F native volume.bin");
            fs::write(&path, b"volume observation does not mutate bytes")?;
            let (file, identity) = metadata_witness(&path, false)?;
            // Unknown is an explicit failed positive qualification, never a
            // skip/pass or substitution with caller paths/drive letters.
            assert_eq!(observe(&file)?, Some(identity));
            let (directory, directory_id) = metadata_witness(roots.library(), true)?;
            assert_eq!(observe(&directory)?, Some(directory_id));
            assert_eq!(query_identity(&file)?.identity, identity);
            assert_eq!(
                fs::read(&path)?,
                b"volume observation does not mutate bytes"
            );
            Ok(())
        }

        #[test]
        fn windows_fixed_ntfs_native_short_buffers_keep_errors_or_truncation_unknown() -> TestResult
        {
            let roots = TestRoots::new()?;
            let path = roots.library().join("owned.bin");
            fs::write(&path, b"unchanged")?;
            let (file, identity) = metadata_witness(&path, false)?;
            let mut short_fs = [0u16; 1];
            assert!(matches!(
                filesystem(&file, &mut short_fs),
                Err(StorageError::Io { .. })
            ));
            assert!(matches!(
                observe_with(&file, &mut Native, 1, PATH_CAPACITY),
                Err(StorageError::Io { .. })
            ));
            let mut short_path = [0u16; 1];
            let required = opened_path(&file, &mut short_path)?;
            assert!(required >= short_path.len() as u32);
            assert!(guid_root(&short_path, required).is_none());
            assert_eq!(observe_with(&file, &mut Native, FS_CAPACITY, 1)?, None);
            assert_eq!(query_identity(&file)?.identity, identity);
            assert_eq!(fs::read(&path)?, b"unchanged");
            Ok(())
        }

        #[derive(Clone, Copy)]
        enum Fault {
            FilesystemUnavailable,
            PathUnavailable,
            Drive(u32),
            DifferentFilesystem,
            ChangedFilesystem,
            ChangedIdentity,
            ChangedGuid,
            MissingPathNul,
            ZeroLength,
            RequiredLength,
            DriveLetterPath,
        }
        struct Controlled {
            fault: Fault,
            facts_calls: usize,
            filesystem_calls: usize,
            path_calls: usize,
            drive_calls: usize,
        }
        impl Calls for Controlled {
            fn facts(&mut self, file: &File) -> Result<ObjectFacts> {
                let mut facts = query_identity(file)?;
                self.facts_calls += 1;
                if matches!(self.fault, Fault::ChangedIdentity) && self.facts_calls == 2 {
                    facts.identity.id[0] ^= 1;
                }
                Ok(facts)
            }
            fn filesystem(&mut self, _file: &File, buffer: &mut [u16]) -> Result<()> {
                self.filesystem_calls += 1;
                if matches!(self.fault, Fault::FilesystemUnavailable) {
                    return Err(StorageError::io(
                        "injected unavailable filesystem observation",
                        io::Error::from(io::ErrorKind::Unsupported),
                    ));
                }
                let name = if matches!(self.fault, Fault::DifferentFilesystem)
                    || (matches!(self.fault, Fault::ChangedFilesystem)
                        && self.filesystem_calls == 2)
                {
                    [82, 101, 70, 83, 0] // ReFS, not an NTFS observation.
                } else {
                    [78, 84, 70, 83, 0]
                };
                buffer[..name.len()].copy_from_slice(&name);
                Ok(())
            }
            fn opened_path(&mut self, _file: &File, buffer: &mut [u16]) -> Result<u32> {
                self.path_calls += 1;
                if matches!(self.fault, Fault::PathUnavailable) {
                    return Err(StorageError::io(
                        "injected unavailable opened path observation",
                        io::Error::from(io::ErrorKind::Unsupported),
                    ));
                }
                let text = if matches!(self.fault, Fault::DriveLetterPath) {
                    "C:\\caller-path\\owned"
                } else if matches!(self.fault, Fault::ChangedGuid) && self.path_calls == 2 {
                    "\\\\?\\Volume{01234567-89ab-cdef-0123-456789abcdee}\\owned"
                } else {
                    "\\\\?\\Volume{01234567-89ab-cdef-0123-456789abcdef}\\owned"
                };
                let units: Vec<_> = OsStr::new(text).encode_wide().collect();
                buffer[..units.len()].copy_from_slice(&units);
                if matches!(self.fault, Fault::MissingPathNul) {
                    buffer[units.len()] = 65;
                }
                Ok(match self.fault {
                    Fault::ZeroLength => 0,
                    Fault::RequiredLength => buffer.len() as u32,
                    _ => units.len() as u32,
                })
            }
            fn drive_type(&mut self, root: &[u16; GUID_ROOT_UNITS + 1]) -> u32 {
                self.drive_calls += 1;
                assert_eq!(guid_root(root, GUID_ROOT_UNITS as u32), Some(*root));
                match self.fault {
                    Fault::Drive(value) => value,
                    _ => DRIVE_FIXED,
                }
            }
        }

        #[test]
        fn windows_fixed_ntfs_controlled_unknown_changed_and_malformed_observations_refuse()
        -> TestResult {
            let roots = TestRoots::new()?;
            let path = roots.library().join("owned.bin");
            fs::write(&path, b"retained")?;
            let (file, identity) = metadata_witness(&path, false)?;
            for fault in [
                Fault::FilesystemUnavailable,
                Fault::PathUnavailable,
                Fault::Drive(DRIVE_UNKNOWN),
                Fault::Drive(DRIVE_NO_ROOT_DIR),
                Fault::Drive(DRIVE_REMOTE),
                Fault::Drive(DRIVE_REMOVABLE),
                Fault::DifferentFilesystem,
                Fault::ChangedFilesystem,
                Fault::ChangedIdentity,
                Fault::ChangedGuid,
                Fault::MissingPathNul,
                Fault::ZeroLength,
                Fault::RequiredLength,
                Fault::DriveLetterPath,
            ] {
                let mut calls = Controlled {
                    fault,
                    facts_calls: 0,
                    filesystem_calls: 0,
                    path_calls: 0,
                    drive_calls: 0,
                };
                assert!(!matches!(
                    observe_with(&file, &mut calls, FS_CAPACITY, PATH_CAPACITY),
                    Ok(Some(_))
                ));
                if matches!(
                    fault,
                    Fault::FilesystemUnavailable
                        | Fault::DifferentFilesystem
                        | Fault::PathUnavailable
                        | Fault::DriveLetterPath
                        | Fault::MissingPathNul
                        | Fault::ZeroLength
                        | Fault::RequiredLength
                ) {
                    assert_eq!(calls.drive_calls, 0);
                }
            }
            let valid: Vec<_> =
                OsStr::new("\\\\?\\Volume{01234567-89ab-cdef-0123-456789abcdef}\\owned")
                    .encode_wide()
                    .chain(Some(0))
                    .collect();
            let length = (valid.len() - 1) as u32;
            assert!(guid_root(&valid, length).is_some());
            for index in [0, 2, 10, 11, 19, 24, 29, 34, 47, 48] {
                let mut changed = valid.clone();
                changed[index] = 71; // Neither punctuation nor hex.
                assert!(guid_root(&changed, length).is_none());
            }
            assert!(!exact_ntfs(&[78, 84, 70, 83])); // No terminating NUL.
            assert!(!exact_ntfs(&[78, 84, 70, 83, 88, 0]));
            assert_eq!(query_identity(&file)?.identity, identity);
            assert_eq!(fs::read(&path)?, b"retained");
            Ok(())
        }
    }
}

// Deliberately test-only: this is an owned, finite API observation, not a
// production directory-flush capability or a mutation/durability precondition.
#[cfg(test)]
mod directory_sync_qualification {
    use super::*;
    use crate::platform::lock::LibraryLease;
    use librairii_testkit::TestRoots;
    use std::{
        error::Error,
        fs,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    use windows_sys::{
        Wdk::Storage::FileSystem::FILE_DIRECTORY_FILE,
        Win32::{Foundation::ERROR_ACCESS_DENIED, Storage::FileSystem::FILE_GENERIC_WRITE},
    };

    type TestResult = std::result::Result<(), Box<dyn Error>>;
    const LIST_ACCESS: u32 = FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
    const WRITE_ACCESS: u32 = FILE_GENERIC_WRITE | LIST_ACCESS;
    const SHARE: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE;
    const OPTIONS: u32 = FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT;
    const SENTINEL: &[u8] = b"owned sentinel: safe sync observes the SAME directory handle";
    const _: () = {
        assert!(LIST_ACCESS == 0x0010_0081);
        assert!(WRITE_ACCESS == 0x0012_0197);
        assert!(SHARE == 3);
        assert!(OPTIONS == 0x21);
        assert!(WRITE_ACCESS & 0x4000_0000 == 0); // No raw GENERIC_WRITE bit.
        assert!(size_of::<OBJECT_ATTRIBUTES>() <= u32::MAX as usize);
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Cut {
        None,
        ChildQuery,
        BeforeClassification,
        UnknownClassification,
        BeforeSync,
        AfterSync,
        AfterClassification,
    }

    fn injected(cut: Cut) -> StorageError {
        StorageError::io(
            "injected directory qualification observation failure",
            io::Error::other(format!("{cut:?}")),
        )
    }

    fn is_injected(error: Option<&StorageError>, cut: Cut) -> bool {
        matches!(error, Some(StorageError::Io { operation, source })
            if *operation == "injected directory qualification observation failure"
                && source.to_string() == format!("{cut:?}"))
    }

    // There is intentionally no Error/From/raw-file extraction adapter. Even a
    // pre-open refusal retains the actual lease; a post-open refusal retains
    // the actual File before attempting any fallible child observation.
    #[derive(Debug)]
    struct OpenAttempt<'lease> {
        file: Option<File>,
        identity: Option<FileIdentity>,
        status: Option<i32>,
        access: u32,
        failure: Option<StorageError>,
        lease: &'lease LibraryLease,
    }
    impl Drop for OpenAttempt<'_> {
        fn drop(&mut self) {}
    }
    impl OpenAttempt<'_> {
        fn checked(&self) -> bool {
            self.status.is_some_and(|status| status >= 0)
                && self.file.is_some()
                && self.identity.is_some()
                && self.failure.is_none()
        }
    }

    /// ONE existing directory child, relative only to the retained lease root.
    ///
    /// # Invariants
    /// All pointers refer to initialized, aligned SDK repr(C) locals and checked
    /// UTF-16 BYTE lengths, live through final synchronous completion. NONALERT
    /// includes SYNCHRONIZE in both fixed masks; no APC/event/async I/O exists.
    /// OBJ_DONT_REPARSE refuses encountered reparse; this distinct directory
    /// variant omits OPEN_REPARSE_POINT without changing the regular-file path.
    /// FILE_OPEN cannot create/supersede. Successful unique ownership is installed
    /// in the attempt immediately, before metadata/type/identity observations.
    #[expect(
        unsafe_code,
        reason = "necessary finite synchronous root-relative directory-open qualification; initialized SDK buffers and unique RAII ownership documented at each call"
    )]
    fn open<'lease>(
        lease: &'lease LibraryLease,
        name: &OsStr,
        access: u32,
        cut: Cut,
    ) -> OpenAttempt<'lease> {
        let mut attempt = OpenAttempt {
            file: None,
            identity: None,
            status: None,
            access,
            failure: None,
            lease,
        };
        let mut units = match checked_child_name(name) {
            Ok(units) => units,
            Err(error) => {
                attempt.failure = Some(error);
                return attempt;
            }
        };
        let (parent, expected_parent) = lease.retained_windows_root();
        match query_identity(parent) {
            Ok(facts) if facts.directory && facts.identity == expected_parent => {}
            Ok(_) => {
                attempt.failure = Some(StorageError::LibraryLeaseChanged);
                return attempt;
            }
            Err(error) => {
                attempt.failure = Some(error);
                return attempt;
            }
        }
        // checked_child_name already bounded units to u16::MAX / 2.
        let bytes = match u16::try_from(units.len() * 2) {
            Ok(bytes) => bytes,
            Err(_) => {
                attempt.failure = Some(StorageError::InvalidManagedPath);
                return attempt;
            }
        };
        let text = UNICODE_STRING {
            Length: bytes,
            MaximumLength: bytes,
            Buffer: units.as_mut_ptr(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.as_raw_handle(),
            ObjectName: &text,
            Attributes: OBJ_DONT_REPARSE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut completion = IO_STATUS_BLOCK {
            // Initialize the entire pointer-width union, not just NTSTATUS.
            Anonymous: IO_STATUS_BLOCK_0 {
                Pointer: std::ptr::null_mut(),
            },
            Information: 0,
        };
        let mut handle = std::ptr::null_mut();
        // SAFETY: actual lease/root File stays borrowed/live, with no close or
        // duplication. All repr(C) locals/name storage are initialized, aligned,
        // unaliased during the synchronous call and live until its final return.
        // Checked name lengths exclude NUL. Optional buffers are NULL/zero.
        // The two fixed specific-rights masks include SYNCHRONIZE; NONALERT
        // requests final synchronous completion, with no pending pointers/APC.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                access,
                &attributes,
                &mut completion,
                std::ptr::null(),
                0,
                SHARE,
                FILE_OPEN,
                OPTIONS,
                std::ptr::null(),
                0,
            )
        };
        // A successful native return supplies the unique owned HANDLE. Install
        // it before any fallible query/classifier/sync or test fault boundary.
        if status >= 0 && !handle.is_null() && handle != INVALID_HANDLE_VALUE {
            // SAFETY: successful synchronous NtCreateFile returned one valid
            // uniquely owned HANDLE. Only this File will close it; no pending
            // operation, duplicate owner or escaped native pointer exists.
            attempt.file = Some(unsafe { File::from_raw_handle(handle) });
        }
        attempt.status = Some(status);
        if status < 0 {
            // The raw failed NTSTATUS IS the open failure, retained above; no
            // fabricated StorageError/BOOL/last-error conversion replaces it.
            return attempt;
        }
        if attempt.file.is_none() {
            attempt.failure = Some(StorageError::CompatibilityInvariant);
            return attempt;
        }
        if cut == Cut::ChildQuery {
            attempt.failure = Some(injected(cut));
            return attempt;
        }
        if let Some(file) = &attempt.file {
            match query_identity(file) {
                Ok(facts) => {
                    attempt.identity = Some(facts.identity);
                    if !facts.directory || facts.identity.volume != expected_parent.volume {
                        attempt.failure = Some(StorageError::PathEscape);
                    }
                }
                Err(error) => attempt.failure = Some(error),
            }
        }
        attempt
    }

    // These are references to physically retained owners in the synchronous
    // host frame. The candidate/negative observations borrow this whole frame;
    // neither a path, expected ID nor a DTO can replace its actual handles.
    #[derive(Debug)]
    struct Pins<'pins, 'lease> {
        list: &'pins OpenAttempt<'lease>,
        sentinel: &'pins File,
        sentinel_id: FileIdentity,
    }
    impl Pins<'_, '_> {
        fn revalidate(&self) -> Result<FileIdentity> {
            let (root, expected_root) = self.list.lease.retained_windows_root();
            let root = query_identity(root)?;
            let list = self
                .list
                .file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            let list = query_identity(list)?;
            let sentinel = query_identity(self.sentinel)?;
            if !self.list.checked()
                || self.list.access != LIST_ACCESS
                || !root.directory
                || root.identity != expected_root
                || !list.directory
                || Some(list.identity) != self.list.identity
                || list.identity.volume != root.identity.volume
                || sentinel.directory
                || sentinel.links != 1
                || sentinel.identity != self.sentinel_id
                || sentinel.identity.volume != root.identity.volume
            {
                return Err(StorageError::LibraryLeaseChanged);
            }
            Ok(list.identity)
        }
    }

    #[derive(Debug)]
    struct SyncAttempt<'pins, 'lease> {
        open: OpenAttempt<'lease>,
        before: Option<FileIdentity>,
        after: Option<FileIdentity>,
        sync: Option<io::Result<()>>,
        refusal: Option<StorageError>,
        post_failure: Option<StorageError>,
        pins: &'pins Pins<'pins, 'lease>,
    }
    impl Drop for SyncAttempt<'_, '_> {
        fn drop(&mut self) {}
    }
    impl SyncAttempt<'_, '_> {
        fn qualified_ack(&self) -> bool {
            self.open.checked()
                && self.open.access == WRITE_ACCESS
                && self.refusal.is_none()
                && self.post_failure.is_none()
                && self.before.is_some()
                && self.before == self.after
                && matches!(self.sync, Some(Ok(())))
        }
        fn qualified_access_denied(&self) -> bool {
            self.open.checked()
                && self.open.access == LIST_ACCESS
                && self.refusal.is_none()
                && self.post_failure.is_none()
                && self.before.is_some()
                && self.before == self.after
                && matches!(&self.sync, Some(Err(error)) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32))
        }
    }

    fn observe<'pins, 'lease>(
        pins: &'pins Pins<'pins, 'lease>,
        name: &OsStr,
        access: u32,
        cut: Cut,
    ) -> SyncAttempt<'pins, 'lease> {
        let mut attempt = SyncAttempt {
            open: open(pins.list.lease, name, access, cut),
            before: None,
            after: None,
            sync: None,
            refusal: None,
            post_failure: None,
            pins,
        };
        if !attempt.open.checked() {
            return attempt;
        }
        let preflight = (|| -> Result<FileIdentity> {
            let expected = pins.revalidate()?;
            let file = attempt
                .open
                .file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            if attempt.open.identity != Some(expected) {
                return Err(StorageError::LibraryLeaseChanged);
            }
            if cut == Cut::BeforeClassification {
                return Err(injected(cut));
            }
            let classification = if cut == Cut::UnknownClassification {
                None
            } else {
                fixed_ntfs::observe(file)?
            };
            if classification != Some(expected) || query_identity(file)?.identity != expected {
                return Err(StorageError::CompatibilityInvariant);
            }
            Ok(expected)
        })();
        match preflight {
            Ok(identity) => attempt.before = Some(identity),
            Err(error) => {
                attempt.refusal = Some(error);
                return attempt;
            }
        }
        if cut == Cut::BeforeSync {
            attempt.refusal = Some(injected(cut));
            return attempt;
        }
        if let Some(file) = &attempt.open.file {
            // SAFE std route, on exactly the retained opened File. No second
            // handle, reopen, FlushFileBuffers FFI or permission fallback.
            attempt.sync = Some(file.sync_all());
        }
        // Even native sync errors retain their independent raw OS error while
        // attempting fresh post-observations. Later failure never erases it.
        let postflight = (|| -> Result<FileIdentity> {
            if cut == Cut::AfterSync {
                return Err(injected(cut));
            }
            let file = attempt
                .open
                .file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            let identity = query_identity(file)?.identity;
            if fixed_ntfs::observe(file)? != Some(identity) {
                return Err(StorageError::CompatibilityInvariant);
            }
            if cut == Cut::AfterClassification {
                return Err(injected(cut));
            }
            if Some(identity) != attempt.before || pins.revalidate()? != identity {
                return Err(StorageError::LibraryLeaseChanged);
            }
            Ok(identity)
        })();
        match postflight {
            Ok(identity) => attempt.after = Some(identity),
            Err(error) => attempt.post_failure = Some(error),
        }
        attempt
    }

    fn retained_sentinel(file: &File) -> io::Result<Vec<u8>> {
        use std::os::windows::fs::FileExt;
        // SAME File, bounded explicit-offset reads, including one growth
        // sentinel byte. No second handle/path reopen or content mutation.
        let mut buffer = [0u8; SENTINEL.len() + 1];
        let mut length = 0;
        while length < buffer.len() {
            match file.seek_read(&mut buffer[length..], length as u64) {
                Ok(0) => break,
                Ok(read) => length += read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(buffer[..length].to_vec())
    }

    fn context(directory: &Path) -> TestResult {
        // Fixed readonly commands, no shell interpolation, ACL/token changes or
        // security FFI. Bound recorded output and refuse command failure; an
        // unavailable executable is explicitly recorded, never a native pass.
        for (program, args) in [
            (
                "cmd.exe",
                vec![OsStr::new("/D"), OsStr::new("/C"), OsStr::new("ver")],
            ),
            ("whoami.exe", vec![OsStr::new("/all")]),
            ("icacls.exe", vec![directory.as_os_str()]),
        ] {
            // Redirect to an anonymous owned working file outside the library
            // namespace: no pipe deadlock or unbounded in-memory capture. Bound
            // the wait and read, and always reap our own context-command child.
            let mut output = tempfile::tempfile()?;
            match Command::new(program)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::from(output.try_clone()?))
                .stderr(Stdio::from(output.try_clone()?))
                .spawn()
            {
                Ok(mut child) => {
                    let start = Instant::now();
                    let status = loop {
                        let outcome = child.try_wait();
                        // A first observed success after the deadline is not
                        // qualified merely because try_wait returned Some.
                        let observed_at = Instant::now();
                        match outcome {
                            Ok(Some(status))
                                if observed_at.duration_since(start) < Duration::from_secs(5) =>
                            {
                                assert_eq!(child.wait()?, status);
                                break status;
                            }
                            Ok(None)
                                if observed_at.duration_since(start) < Duration::from_secs(5) =>
                            {
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            outcome => {
                                let killed = child.kill();
                                let reaped = child.wait();
                                return Err(io::Error::other(format!(
                                    "readonly {program} did not complete: {outcome:?}; kill={killed:?}, reap={reaped:?}"
                                )).into());
                            }
                        }
                    };
                    use std::io::{Seek, SeekFrom};
                    output.seek(SeekFrom::Start(0))?;
                    let mut bytes = Vec::new();
                    output.take(64 * 1024 + 1).read_to_end(&mut bytes)?;
                    assert!(bytes.len() <= 64 * 1024, "context output exceeded bound");
                    // Native console output can use a legacy code page. Retain
                    // the exact bytes as well as a human-readable rendering.
                    eprintln!(
                        "{program}: status={status} bytes={bytes:02x?} text={}",
                        String::from_utf8_lossy(&bytes)
                    );
                    assert!(status.success(), "readonly context command failed");
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    eprintln!("{program}: UNAVAILABLE {error}");
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    #[test]
    fn windows_directory_safe_sync_owned_write_ack_requires_list_access_denied_contrast()
    -> TestResult {
        let roots = TestRoots::new()?;
        let name = OsStr::new("owned Éte # %2F directory");
        let directory = roots.library().join(name);
        fs::create_dir(&directory)?;
        let sentinel_path = directory.join("sentinel.bin");
        fs::write(&sentinel_path, SENTINEL)?;
        let lease = LibraryLease::create_fresh(roots.library())?;
        context(&directory)?;
        let list = open(&lease, name, LIST_ACCESS, Cut::None);
        assert!(list.checked(), "{list:?}");
        let (sentinel, sentinel_id) = metadata_witness(&sentinel_path, false)?;
        let pins = Pins {
            list: &list,
            sentinel: &sentinel,
            sentinel_id,
        };
        let candidate = observe(&pins, name, WRITE_ACCESS, Cut::None);
        let negative = observe(&pins, name, LIST_ACCESS, Cut::None);
        eprintln!(
            "candidate_access={WRITE_ACCESS:#x} negative_access={LIST_ACCESS:#x} share={SHARE:#x} disposition={FILE_OPEN:#x} options={OPTIONS:#x} object_attributes={OBJ_DONT_REPARSE:#x}\ncandidate={candidate:?}\nnegative={negative:?}"
        );
        // Unsupported/Unknown, unexpected negative ACK or any other error is a
        // FAILED qualification. No skip-success or numeric/generic fallback.
        assert!(candidate.qualified_ack(), "{candidate:?}");
        assert!(negative.qualified_access_denied(), "{negative:?}");
        assert_eq!(candidate.before, negative.before);
        let bytes = retained_sentinel(pins.sentinel)?;
        assert_eq!(bytes, SENTINEL);
        assert_eq!(query_identity(pins.sentinel)?.identity, sentinel_id);
        lease.revalidate()?;
        eprintln!(
            "same_handle_classification_before_and_after=NTFS+volume_GUID+DRIVE_FIXED sentinel_id={sentinel_id:?} sentinel_bytes={bytes:02x?}; directory_durability=UNQUALIFIED"
        );
        Ok(())
    }

    #[test]
    fn windows_directory_safe_sync_refuses_missing_wrong_kind_reparse_and_invalid_components()
    -> TestResult {
        let roots = TestRoots::new()?;
        let directory = roots.library().join("owned");
        fs::create_dir(&directory)?;
        fs::write(directory.join("sentinel.bin"), SENTINEL)?;
        fs::write(roots.library().join("regular.bin"), b"unchanged regular")?;
        // Required native fixture construction, never silently skipped.
        std::os::windows::fs::symlink_dir(&directory, roots.library().join("reparse"))?;
        let lease = LibraryLease::create_fresh(roots.library())?;
        let list = open(&lease, OsStr::new("owned"), LIST_ACCESS, Cut::None);
        assert!(list.checked(), "{list:?}");
        let (sentinel, sentinel_id) = metadata_witness(&directory.join("sentinel.bin"), false)?;
        let pins = Pins {
            list: &list,
            sentinel: &sentinel,
            sentinel_id,
        };
        let count = fs::read_dir(roots.library())?.count();
        for name in [
            "missing",
            "regular.bin",
            "reparse",
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "C:drive",
            "nul\0tail",
        ] {
            let attempt = observe(&pins, OsStr::new(name), WRITE_ACCESS, Cut::None);
            assert!(!attempt.qualified_ack(), "{name:?}: {attempt:?}");
            assert!(attempt.sync.is_none(), "{attempt:?}");
            assert!(std::ptr::eq(attempt.pins.list.lease, &lease));
            eprintln!("refused {name:?}: {attempt:?}");
        }
        assert_eq!(fs::read_dir(roots.library())?.count(), count);
        assert_eq!(retained_sentinel(pins.sentinel)?, SENTINEL);
        assert_eq!(
            fs::read(roots.library().join("regular.bin"))?,
            b"unchanged regular"
        );
        lease.revalidate()?;
        Ok(())
    }

    #[test]
    fn windows_directory_safe_sync_faults_retain_acquired_handle_pins_and_independent_errors()
    -> TestResult {
        let roots = TestRoots::new()?;
        let name = OsStr::new("owned");
        let directory = roots.library().join(name);
        fs::create_dir(&directory)?;
        fs::write(directory.join("sentinel.bin"), SENTINEL)?;
        let lease = LibraryLease::create_fresh(roots.library())?;
        let list = open(&lease, name, LIST_ACCESS, Cut::None);
        assert!(list.checked(), "{list:?}");
        let (sentinel, sentinel_id) = metadata_witness(&directory.join("sentinel.bin"), false)?;
        let pins = Pins {
            list: &list,
            sentinel: &sentinel,
            sentinel_id,
        };
        let expected = pins.revalidate()?;
        for cut in [
            Cut::ChildQuery,
            Cut::BeforeClassification,
            Cut::UnknownClassification,
            Cut::BeforeSync,
            Cut::AfterSync,
            Cut::AfterClassification,
        ] {
            let attempt = observe(&pins, name, LIST_ACCESS, cut);
            assert!(!attempt.qualified_ack(), "{attempt:?}");
            let file = attempt
                .open
                .file
                .as_ref()
                .ok_or("fault lost actual acquired handle")?;
            assert_eq!(query_identity(file)?.identity, expected);
            assert!(std::ptr::eq(attempt.pins, &pins));
            if cut == Cut::ChildQuery {
                assert_eq!(attempt.open.identity, None);
                assert!(is_injected(attempt.open.failure.as_ref(), cut));
            } else {
                assert_eq!(attempt.open.identity, Some(expected));
                assert!(attempt.open.checked(), "{attempt:?}");
            }
            if matches!(cut, Cut::AfterSync | Cut::AfterClassification) {
                assert!(
                    matches!(&attempt.sync, Some(Err(error)) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32)),
                    "{attempt:?}"
                );
                assert!(is_injected(attempt.post_failure.as_ref(), cut));
            } else {
                assert!(attempt.sync.is_none());
                if matches!(cut, Cut::BeforeClassification | Cut::BeforeSync) {
                    assert!(is_injected(attempt.refusal.as_ref(), cut));
                } else if cut == Cut::UnknownClassification {
                    assert!(matches!(
                        attempt.refusal,
                        Some(StorageError::CompatibilityInvariant)
                    ));
                }
            }
            assert_eq!(retained_sentinel(pins.sentinel)?, SENTINEL);
            eprintln!("retained cut={cut:?}: {attempt:?}");
        }
        lease.revalidate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use librairii_testkit::TestRoots;
    use std::{
        error::Error,
        fs,
        io::{Read, Write},
    };
    type TestResult = std::result::Result<(), Box<dyn Error>>;

    #[test]
    fn windows_relative_read_retains_actual_lease_and_exact_native_name() -> TestResult {
        let roots = TestRoots::new()?;
        let name = OsStr::new("Éte 🦀 # %2F.bin");
        let path = roots.library().join(name);
        fs::write(&path, b"retained bytes")?;
        let lease = super::super::lock::LibraryLease::create_fresh(roots.library())?;
        let mut child = lease.read_child(name)?;
        let mut bytes = [0; 14];
        child.read_exact(&mut bytes)?;
        assert_eq!(&bytes, b"retained bytes");
        assert_eq!(
            child.identity(),
            query_identity(&File::open(&path)?)?.identity
        );
        let renamed = roots.library().join("renamed.bin");
        assert!(fs::rename(&path, &renamed).is_err());
        drop(child);
        fs::rename(&path, &renamed)?;
        assert_eq!(fs::read(&renamed)?, b"retained bytes");
        // The actual locked file is revalidated via the SAME relative NT open;
        // querying it does not read through the held byte-range lock.
        lease.revalidate()?;
        Ok(())
    }

    #[test]
    fn windows_relative_read_refuses_unowned_paths_aliases_and_length_without_creation()
    -> TestResult {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let roots = TestRoots::new()?;
        let lease = super::super::lock::LibraryLease::create_fresh(roots.library())?;
        let before = fs::read_dir(roots.library())?.count();
        for name in [
            "",
            ".",
            "..",
            "/absolute",
            "\\absolute",
            "C:drive",
            "a/b",
            "a\\b",
            "nul\0tail",
        ] {
            assert!(matches!(
                lease.read_child(OsStr::new(name)),
                Err(StorageError::InvalidManagedPath)
            ));
        }
        let too_long = OsString::from_wide(&vec![65; (u16::MAX as usize / 2) + 1]);
        assert!(matches!(
            lease.read_child(&too_long),
            Err(StorageError::InvalidManagedPath)
        ));
        let raw_units = [0xD800, 95, 0xDC00];
        assert_eq!(
            checked_child_name(&OsString::from_wide(&raw_units))?,
            raw_units
        );
        let missing = lease
            .read_child(OsStr::new("missing"))
            .err()
            .ok_or("missing file opened")?;
        assert!(missing.to_string().contains("NTSTATUS 0x"));
        assert_eq!(fs::read_dir(roots.library())?.count(), before);
        fs::create_dir(roots.library().join("directory"))?;
        assert!(lease.read_child(OsStr::new("directory")).is_err());
        let original = roots.library().join("original");
        let alias = roots.library().join("alias");
        fs::write(&original, b"preserved")?;
        fs::hard_link(&original, &alias)?;
        assert!(matches!(
            lease.read_child(OsStr::new("alias")),
            Err(StorageError::PathEscape)
        ));
        fs::remove_file(&alias)?;
        let foreign = roots.profile().join("foreign");
        fs::write(&foreign, b"never followed")?;
        // Required native evidence, never a skip if link creation is denied.
        std::os::windows::fs::symlink_file(&foreign, &alias)?;
        assert!(lease.read_child(OsStr::new("alias")).is_err());
        assert_eq!(fs::read(&foreign)?, b"never followed");
        assert_eq!(fs::read(&original)?, b"preserved");
        Ok(())
    }

    #[test]
    fn windows_query_retains_volume_and_128bit_id_across_handles_and_content_changes() -> TestResult
    {
        let roots = TestRoots::new()?;
        let path = roots.library().join("Éte # %2F.bin");
        fs::write(&path, b"first bytes")?;
        let file = File::open(&path)?;
        let first = query_identity(&file)?;
        let (pin, identity) = metadata_witness(&path, false)?;
        assert_eq!(first.identity, identity);
        assert!(!first.directory);
        assert_eq!(first.links, 1);
        let mut writer = OpenOptions::new().write(true).open(&path)?;
        writer.write_all(b"new bytes with same live object")?;
        writer.flush()?;
        assert_eq!(query_identity(&file)?.identity, identity);
        assert_eq!(query_identity(&pin)?.identity, identity);
        Ok(())
    }

    #[test]
    fn windows_metadata_pins_block_delete_rename_until_actual_handle_release() -> TestResult {
        let roots = TestRoots::new()?;
        let path = roots.library().join("owned.bin");
        let renamed = roots.library().join("renamed.bin");
        fs::write(&path, b"recoverable")?;
        let file = File::open(&path)?;
        // The original attribute-only handle observes identity but does not
        // establish a no-delete-sharing pin. Keep this control distinct from
        // the actual readable pin and its unchanged exclusion assertions.
        let attributes = OpenOptions::new()
            .read(true)
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(&path)?;
        let original = query_identity(&attributes)?.identity;
        fs::rename(&path, &renamed)?;
        assert_eq!(query_identity(&attributes)?.identity, original);
        fs::rename(&renamed, &path)?;
        drop(attributes);

        let (mut pin, identity) = metadata_witness(&path, false)?;
        let mut bytes = [0; 11];
        pin.read_exact(&mut bytes)?;
        assert_eq!(&bytes, b"recoverable");
        assert_eq!(identity, original);
        assert!(fs::rename(&path, &renamed).is_err());
        assert!(fs::remove_file(&path).is_err());
        assert_eq!(fs::read(&path)?, b"recoverable");
        drop(pin);
        fs::rename(&path, &renamed)?;
        assert_eq!(query_identity(&file)?.identity, identity);
        fs::write(&path, b"replacement name")?;
        assert_ne!(metadata_witness(&path, false)?.1, identity);
        assert_eq!(fs::read(&renamed)?, b"recoverable");

        let directory = roots.library().join("retained-directory");
        let moved_directory = roots.library().join("moved-directory");
        fs::create_dir(&directory)?;
        let (pin, identity) = metadata_witness(&directory, true)?;
        assert!(fs::rename(&directory, &moved_directory).is_err());
        assert!(fs::remove_dir(&directory).is_err());
        assert_eq!(query_identity(&pin)?.identity, identity);
        drop(pin);
        fs::rename(&directory, &moved_directory)?;
        assert_eq!(metadata_witness(&moved_directory, true)?.1, identity);
        fs::remove_dir(&moved_directory)?;
        Ok(())
    }

    #[test]
    fn windows_metadata_refuses_hardlink_and_reparse_aliases_without_following_them() -> TestResult
    {
        let roots = TestRoots::new()?;
        let original = roots.library().join("original.bin");
        let alias = roots.library().join("hardlink.bin");
        let link = roots.library().join("symlink.bin");
        fs::write(&original, b"outside bytes stay untouched")?;
        fs::hard_link(&original, &alias)?;
        assert_eq!(query_identity(&File::open(&original)?)?.links, 2);
        assert_eq!(
            query_identity(&File::open(&original)?)?.identity,
            query_identity(&File::open(&alias)?)?.identity
        );
        assert!(matches!(
            metadata_witness(&alias, false),
            Err(StorageError::PathEscape)
        ));
        fs::remove_file(alias)?;
        // Required native evidence; inability to create this synthetic link is
        // an explicit test failure, never a skip or reparse qualification pass.
        std::os::windows::fs::symlink_file(&original, &link)?;
        assert!(matches!(
            metadata_witness(&link, false),
            Err(StorageError::PathEscape)
        ));
        assert_eq!(fs::read(&original)?, b"outside bytes stay untouched");
        Ok(())
    }

    #[test]
    fn windows_metadata_handles_directory_kind_wrong_type_missing_and_concurrent_queries()
    -> TestResult {
        let roots = TestRoots::new()?;
        let path = roots.library().join("file.bin");
        fs::write(&path, b"query concurrency")?;
        let (directory, directory_id) = metadata_witness(roots.library(), true)?;
        assert!(query_identity(&directory)?.directory);
        assert_eq!(query_identity(&directory)?.identity, directory_id);
        assert!(metadata_witness(&path, true).is_err());
        assert!(metadata_witness(roots.library(), false).is_err());
        assert!(metadata_witness(&roots.library().join("missing"), false).is_err());
        let file = File::open(&path)?;
        let identity = query_identity(&file)?.identity;
        std::thread::scope(|scope| -> TestResult {
            let jobs: Vec<_> = (0..4)
                .map(|_| scope.spawn(|| query_identity(&file)))
                .collect();
            for job in jobs {
                assert_eq!(
                    job.join().map_err(|_| "query thread panic")??.identity,
                    identity
                );
            }
            Ok(())
        })?;
        Ok(())
    }
}
