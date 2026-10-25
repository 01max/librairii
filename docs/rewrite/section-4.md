# Task 4: storage, profiles and database compatibility

Completion branch starts at integration `244f4494e2bf1fcc5cc31c8caba61171ca5f8771`.
The historical PR43 ledger in `crates/librairii-storage/evidence.md` retains its
original commits/results; later sections also strengthened its mechanisms.
This pass inventories that admitted implementation, closes the remaining
query/portability/interoperability evidence gaps, and obtains current native
verification. Windows remains deferred.

## Acceptance map

| Task | Storage implementation and executable evidence | Parity subcases |
| --- | --- | --- |
| 4.1 | Explicit `IsolatedLibrary`, layout/relative managed paths/raw marker checks; `layout_identity` tests refuse missing overrides, aliases and nonfresh roots before profile discovery. | P01.layout/override |
| 4.2 | Exact embedded migrations 001–018, frozen `librairii`/`wails-go-sqlite` identity and contiguous ledger; `migrations` tests compare original SQL bytes and refuse forged DDL, foreign/future/gapped ledgers. | P02.identity/migrations |
| 4.3 | One owned writer/four readers; WAL/FK/5000ms, bounded queues/results and reserved same-writer repair; `database_workers`, worker/closure tests cover snapshots, rollback/commit uncertainty, backpressure and joins. | P02.workers |
| 4.4 | `collection_compatibility` opens all eighteen populated prefixes, compares full schema/constraints/triggers/domain rows/Collection projection with the frozen Go snapshots, and now compares 216 actual Count/Search returns. `query_differential` retains the separate full-scope corpus. | P02.upgrade/query |
| 4.5 | `backup` integration/unit tests cover uncheckpointed WAL, standalone online backups/integrity/FKs, migration failure restoration and retained candidate/backup bytes; foreign DB/WAL/SHM publication is durable before originals are unlinked. | P03.backup/rollback/preservation |
| 4.6 | Strict bounded version-1 independent profiles; serialized atomic updates, separate native profile/recovery namespace, invalid/unknown documents preserved; `profiles` and native profile-failure tests. | P52.profile/namespace |
| 4.7 | OS lease retained through actual drain/close, fail-closed in-place adoption, verified independent copy/native authority, process-death/restart recovery, retarget refusal; `library_locks`, `adoption_inputs`, `native_library`, and the new three-target frozen-Go storage rehearsal. | P01.ownership, P55.isolation, P60.storage |
| 4.8 | Separate descriptor-relative no-clobber publication, owned-config exchange and file/directory durability; Unix tests exercise creation races, substitution, flush failures, retained owners and owned-temp retirement. | P04.publication, P52.atomic-update |
| 4.10 | Actual target literal-name creation/enumeration/hash/durability; UUID/raw-marker-bound native rebinding. New streamed path diagnostics and native case/decomposition races report affected items while preserving bytes and leases. `portability`, `adoption_inputs`, copy failure tests. | P55.names/paths/rebinding |
| 4.11 | Exact-current cumulative native checks, preserved migration/managed hashes, the independent Standards/Spec review and provider receipt below. | P01–P03/P52/P55 storage admission |

These storage subcases do not close whole UI, adoption or release parity rows.
Sections 13 and 17 own the successful settings/relocation and complete Go bridge
handover journeys; package/clean-host acceptance remains Section 18.

## New migration-query evidence (4.4)

`scripts/capture-storage-query-oracle.py` materializes a private module containing
only source bytes admitted by `preservation/oracle-source.json`, refusing changed
members and excluding extra checkout files and inherited Go overrides. It runs
the unchanged Go services on disposable copies of all eighteen prefix DBs.
The new fixture records exact producer/source/input identities. Full Count/Page
JSON is compared through public Rust database workers without normalizing
semantic fields. Only the already admitted synthetic timestamp policy applies.
The six requests exercise default paging, accent-insensitive and literal search,
out-of-range paging/sort metadata, live boolean references and nested OR.

No frozen fixture, migration or Go product source is changed. The existing
five-story schema-018 input and all original archive hashes remain authoritative.

## Path and filename diagnostics (4.10)

`ClosedLibraryInput::visit_portability_paths` reads a newly hash/schema-verified
private database snapshot, never the source SQLite file. It streams the exact
profile root and historical export destinations with raw IDs/path bytes. Each
row retains the existing 16 MiB budget; cancellation/consumer failure refuses
without rewriting source data. Profile and foreign relocation-path diagnostics
recognize syntax only. They do not validate a journal or grant local path,
cleanup, mutation or rebinding authority. Unsupported journals remain preserved
and refused by native binding.

An independent-copy failure exposes its current declared relative item, while
retaining its original cause, actual directory/file owners and lease. Real
target-file races use case and composed/decomposed Unicode alternatives. A
folding filesystem can refuse exclusive creation; a literal filesystem refuses
the extra unowned name. Both preserve the sentinel/source and prevent activation.
This is observed native name-race behavior, not a universal filesystem theorem.

## Shared protocol and isolated legacy storage rehearsal (4.7)

`python3 -I -B scripts/verify-storage-handover.py` builds a small synthetic driver
against the manifest-only frozen Go sources and runs the Rust `storage_handover`
example. The native quality workflow runs it on Linux x86-64, ARM macOS and real
Intel macOS and retains `target/quality/storage-handover.json` with tool/source/
driver/helper identities. Child observations have deadlines and owned kill/wait.

The protocol uses `.librairii-library.lock`, an exclusive nonblocking `flock`
held by the actual regular file through resource closure. Go observes Rust's
held lock; Rust refuses an actual Go owner; killing and reaping that owner
releases the kernel lock without deleting its filename.

The fallback rehearsal independently copies the schema-018 library, binds a
separate native profile/data namespace, opens the real workers, and persists
a new valid pre-move removal intent after startup reconciliation. It invokes
the unmodified Go Bootstrap/Reconcile/ValidateLibrary/database services through
the retained **source** profile and requires an actual source database mutation.
Candidate database/sidecars/archives/intent/native-state/profile bytes must stay
identical. It repeats after native close/reopen and verifies all five immutable
archive hashes in both roots. Its own pre-move intent is explicitly settled
without a custody effect before real close; the product's close refuses an
unsettled removal record.

This is unmodified frozen Go **storage-service** execution through a synthetic
driver, not a launch of the full legacy GUI. It qualifies the shared protocol
and supported storage fallback. Complete Go bridge integration/preferences and
live application relocation stay in Sections 17/13. In-place production adoption
is unconditionally refused; this cooperative lock never excludes an older
executable that ignores it. Existing recovery/retarget tests preserve changed,
foreign and unverified journal/path data without guessing cleanup authority.

## Current verification and independent review

Baseline at `244f449`: full ARM `cargo xtask check` passed 536 nextest tests,
zero skips, all doctests and source/preservation/dependency/build/Clippy gates.
The first sandboxed invocation could not lock the readonly advisory cache;
the authorized invocation with the pinned tools completed.

The new 4.4 six-test suite passed, including all eighteen prefixes and 216 API
comparisons. The affected portability/copy suite passed 89 tests with zero skips,
and storage all-target Clippy passed. Two Python checks prove changed source
refusal, exclusion of added Go files/build overlays and owned materialization
cleanup. Final-head cumulative/native/provider results and independent review
are recorded when complete; these affected-host results do not qualify Intel
or Linux by inference.
