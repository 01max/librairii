//! Private Windows handle observations. No path/data constructor here grants
//! mutation, no-clobber, configuration replacement or directory durability.
use crate::{Result, StorageError};
use std::{
    fs::{File, OpenOptions},
    mem::{align_of, offset_of, size_of},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_BASIC_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_128, FILE_ID_INFO,
    FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    FileAttributeTagInfo, FileBasicInfo, FileIdInfo, FileStandardInfo,
    GetFileInformationByHandleEx,
};

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

/// Read-only terminal-object observation and no-delete-sharing pin, NOT a
/// descriptor-relative ancestry/mutation capability. No reparse fallback.
pub(crate) fn metadata_witness(path: &Path, directory: bool) -> Result<(File, FileIdentity)> {
    let file = OpenOptions::new()
        .read(true)
        .access_mode(FILE_READ_ATTRIBUTES)
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
    use std::{error::Error, fs, io::Write};
    type TestResult = std::result::Result<(), Box<dyn Error>>;

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
        let (pin, identity) = metadata_witness(&path, false)?;
        assert!(fs::rename(&path, &renamed).is_err());
        assert!(fs::remove_file(&path).is_err());
        assert_eq!(fs::read(&path)?, b"recoverable");
        drop(pin);
        fs::rename(&path, &renamed)?;
        assert_eq!(query_identity(&file)?.identity, identity);
        fs::write(&path, b"replacement name")?;
        assert_ne!(metadata_witness(&path, false)?.1, identity);
        assert_eq!(fs::read(&renamed)?, b"recoverable");
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
