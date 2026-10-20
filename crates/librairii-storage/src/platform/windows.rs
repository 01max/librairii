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
