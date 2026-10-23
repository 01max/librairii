# Windows storage handover

## v1 decision — 2026-10-21

v1 supports Linux and macOS only. Windows production storage writes are
unsupported and deferred. Windows CI success does not change that decision.
This handover records existing implementation and evidence; it introduces no
Windows capability, recovery permission, schema change, or qualification.

The reference implementation is PR43 head
[`5541c8bc`](https://github.com/01max/librairii/tree/5541c8bc00686f77f47468448f36399456ffc6f3).
Later work must record its own source revision and native results.
All source links below resolve inside this repository unless otherwise stated.

Production refusals must remain explicit:

- [`backup.rs`](../crates/librairii-storage/src/backup.rs),
  `require_directory_durability`, returns `DirectoryDurabilityUnavailable`
  outside Unix. Backup publication and upgrades must not bypass this check.
- [`profile.rs`](../crates/librairii-storage/src/profile.rs), native profile
  creation and `open_native`, require that directory-durability support.
- The Windows test-only OPEN/sync qualification is not a production write path.
  Unsupported operations must retain their actual owners and uncertain state.

## Remaining work, in priority order

1. **Owned configuration and backup replacement.** Establish a supported
   replacement protocol that preserves readable old/new bytes, backup custody,
   identity, no-follow checks, sharing compatibility, and partial-failure truth.
   Relevant callers: `profile.rs` and `backup.rs`; platform implementation belongs
   in [`platform/windows.rs`](../crates/librairii-storage/src/platform/windows.rs).
2. **Ordered namespace persistence.** Connect a supported local API and access
   contract to file flush, create/rename, and each affected parent-directory
   completion. Same-parent and cross-parent moves need explicit ordering and
   retained failure owners. A successful directory `sync_all` is insufficient.
3. **Production create and no-clobber move.** Implement handle-relative creation
   and collision-safe movement with retained ancestors, exact child names,
   identity checks and refusal of substitutions, links and reparses. Existing
   readonly child operations do not supply this mutation protocol.
4. **Native profile lifecycle.** Qualify native activation, persistent-state
   transitions, restart, recovery refusal/rebinding, relocation and close on
   Windows. Relevant modules: `native.rs`, `profile.rs`, `adoption.rs`,
   `database.rs`, `lock.rs` and `platform/**` under `crates/librairii-storage/src`.
5. **Supported desktop environment.** Establish standard-user Windows 11
   interactive, packaging and accessibility evidence; hosted Server CI is a
   different environment. No verified nonadmin qualification is currently held.

## What the current Windows run establishes

[Run 39534576586, Windows job 119008384492](https://github.com/01max/librairii/actions/runs/39534576586/job/119008384492)
passed on Windows Server 2025 Datacenter, build 10.0.26100, using Rust 1.99.1
MSVC. It executed **109/109 tests, zero skips**, plus three compile-fail docs.
The GitHub test-merge parents were the base `4383373` and head `5541c8bc`;
its tree was exactly the `5541c8bc` tree.

This is finite evidence for the assertions actually executed on that host.
It does not establish Windows production writes, directory durability, crash or
power-loss persistence, Windows 11 desktop behavior, or a standard-user token.

The three new `directory_sync_qualification` cases are:

| Exact test function | Result | Assertion scope |
| --- | --- | --- |
| `windows_directory_safe_sync_owned_write_ack_requires_list_access_denied_contrast` | PASS, 0.245 s | Same retained File sync succeeds for the write candidate; LIST-only contrast returns actual access denied. |
| `windows_directory_safe_sync_refuses_missing_wrong_kind_reparse_and_invalid_components` | PASS, 0.140 s | Finite missing/type/reparse/name refusals retain the fixture contract. |
| `windows_directory_safe_sync_faults_retain_acquired_handle_pins_and_independent_errors` | PASS, 0.109 s | Injected post-acquisition failures retain handles/pins and independent causes. |

These cases use the actual library lease, retained root, one checked native
`OsStr` component, a pinned child directory and a readable sentinel.
The positive access mask is `0x00120197`; the negative is `0x00100081`.
Both use share READ|WRITE without DELETE, `FILE_OPEN`, options `0x21`
(`DIRECTORY_FILE|SYNCHRONOUS_IO_NONALERT`), and `OBJ_DONT_REPARSE`.
Only this distinct qualification variant omits `OPEN_REPARSE_POINT`.
This is not a general incompatibility finding about that option.

Each valid returned File is owned before querying/classifying it. The test
syncs that same File, rechecks identity/classification, and retains a sync error
independently from any later observation error. Qualification requires both the
positive acknowledgement and the actual LIST-only access-denied contrast.

Successful-test stdout is absent from the primary log: it contains no captured
candidate masks, IDs, GUIDs, token, elevation, group, privilege or ACL details.
The assertions passed; raw host values and nonadmin status cannot be inferred.
Bounded context commands have wait/reap and late-observation refusal handling,
but their presence in source does not provide missing captured token evidence.

## Other successful finite Windows evidence

The same source contains passing readonly NtChild, metadata and classifier
tests. These establish their checked local behavior, not mutation authority:

- `windows_relative_read_retains_actual_lease_and_exact_native_name` and
  `windows_relative_read_refuses_unowned_paths_aliases_and_length_without_creation`
  exercise anchored readonly acquisition and finite refusals.
- Four `windows_metadata_*` / `windows_query_*` cases exercise volume plus
  128-bit identity, retained sharing pins, hardlink/reparse refusal, directory
  kind checks and concurrent queries. Identity is not permanent after deletion.
- Three `windows_fixed_ntfs_*` cases exercise the same-handle classifier,
  native short buffers, and controlled unknown/changed/malformed observations.

The classifier uses `GetVolumeInformationByHandleW` for exact NTFS,
`GetFinalPathNameByHandleW` with OPENED|GUID, and `GetDriveTypeW` on only the
derived complete GUID root. It accepts `0 < len < capacity` with a terminator
at `buffer[len]`; truncation, unknown drive types and identity changes refuse.
Positive fixed-NTFS observations are current, multi-call OS observations.
They do not prove physical permanence, custody, or directory persistence.

## Attempts, failures and approaches still unproven

### Attribute-only sharing pins — actual failure, fixed

[The 65ff Windows job](https://github.com/01max/librairii/actions/runs/39504409923/job/118909880578)
failed the metadata pin assertion: rename succeeded while an attribute-only
witness was held. Attribute access alone did not provide the intended exclusion.
[`6c234b9`](https://github.com/01max/librairii/commit/6c234b9cb298c62c9819b6359fd855085d320e71)
changed file pins to READ_DATA and directory pins to LIST_DIRECTORY, retaining
READ_ATTRIBUTES and share READ|WRITE without DELETE. The attribute-only negative
control and rename/delete assertions remain; those cases pass at `5541c8bc`.

### Directory OPEN plus safe sync — finite acknowledgement only

[`9c752fa`](https://github.com/01max/librairii/commit/9c752fa32bb27ab729049419dcecd2e0d211cf63)
introduced the three owned qualification cases above. No new flush FFI was used:
safe `File::sync_all` acts on the retained File. This did not implement create,
move, configuration replacement, backup publication or a both-parent protocol.

[MS-FSA directory flush](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/0de7dc40-9627-437e-a4df-c4696cdc3d02)
documents NTFS persistence; its
[filesystem caveat](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/4e3695bd-7574-4f24-a223-b4679c065b63)
allows other filesystems to return success without that action. The missing
local caller/access/API bridge must be resolved; it is incorrect to claim that
Windows has no documented directory-persistence semantics at all.

### ReplaceFileW — source inference, not a measured sharing failure

[ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)
documents an internal NEW-file open with no sharing mode specified. Its numeric
share mask was not established. **If** that means share mask zero, retained NEW
READ/DELETE custody handles conflict reciprocally even when those handles share
everything. This is a source inference, not an executed ReplaceFileW assay.

OLD sharing compatibility alone does not solve NEW or backup readable custody.
An attribute-only handle cannot substitute for retained readable bytes.
Do not close NEW witnesses to force the operation through. No supported
alternative satisfying the full ownership and persistence contract is held.
The documented partial failures can leave different original/replacement/backup
names; false is not proof of no effect. `REPLACEFILE_WRITE_THROUGH` is unsupported.

### Candidates not implemented or probed as production protocols

- [FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info)
  permits relative RootDirectory and `ReplaceIfExists=false`; this is a candidate
  contract, not native no-clobber race/crash or both-parent persistence proof.
- POSIX replacement flags, `NtFlushBuffersFile`, `NtFlushBuffersFileEx`, volume
  flush, privilege changes and alternative backup/rename sequences were not
  implemented or probed as solutions. Binding availability is not acceptance.
- [FlushFileBuffers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)
  requires write-capable access; a file flush or MoveFileEx WRITE_THROUGH alone
  does not establish the required directory/rename ordering.

### Original 954 worker failure — cause remains unresolved

[Run 39526500284, job 118982600201](https://github.com/01max/librairii/actions/runs/39526500284/job/118982600201)
reported 105/106 passing; only
`four_coherent_read_snapshots_run_concurrently_with_the_separate_writer`
failed with `RuntimeFenced` after 6.137 s. Phase diagnostics were added in
[`4a3ed884`](https://github.com/01max/librairii/commit/4a3ed8847e2aca584d32a6cc7b44304bec3e5e57).
Later passing runs do not diagnose or fix that original failure. No established
timing, infrastructure or platform-specific cause should be recorded.

## Dependency and unsafe boundary

The pinned `windows-sys = 0.61.2`, default features disabled, enables
`Wdk_Foundation`, `Wdk_Storage_FileSystem`, `Win32_Foundation`, `Win32_Security`,
`Win32_Storage_FileSystem`, `Win32_System_IO`, `Win32_System_WindowsProgramming`.
`windows-link` remains 0.2.1. These are raw bindings, not a qualified safe wrapper.
Unsafe exceptions stay function-local with initialized/aligned/sized buffers,
checked UTF-16 byte lengths, synchronous pointer lifetimes, NTSTATUS versus BOOL
handling, and unique RAII handle transfer. No blanket lint suppression is allowed.
See [NtCreateFile](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile)
and [CreateFile sharing](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).

## Ordered resume and acceptance checklist

1. Select an explicit future Windows scope; preserve current production refusals
   until the complete selected path has qualified implementations and callers.
2. Close the replacement custody/access contract and directory persistence API
   gap first. Record same-parent/cross-parent ordering and every uncertain outcome.
3. Implement anchored create/no-clobber move and config/backup callers with actual
   retained ancestors, Files, IDs and readable old/new/backup witnesses.
4. Test collisions, reparses, substitutions, links, sharing conflicts, flush and
   partial failures on the new exact head. Unknown/error must retain owners and
   fences; no cleanup, overwrite or copy fallback may manufacture success.
5. Establish verified standard-user Windows 11 context and qualify native
   profile/state restart, checked relocation/reopen and supported recovery paths.
   Foreign records or recovery data alone must never mint runtime authority.
6. Prove intake stop, required resolution, true owner joins, DB checkpoint/close,
   and filesystem-owner drain before releasing generation/root/profile leases.
   Deadline expiry, dropped receipts or a sync acknowledgement are not settlement.
7. Run necessary exact-head native checks and independent review, then qualify
   desktop packaging, interactive behavior and accessibility before admission.

Unix portability preflight and applied recovery/rebinding, final schema018
managed-archive preservation, and outstanding cross-platform effect/lifetime
contracts retain their own gates. They are not reclassified as Windows-only.
