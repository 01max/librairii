# Task 12: exports, trash custody and diagnostics

**Partial: 12.1–12.7 implemented; 12.8–12.9 remain incomplete. Keep the Task 12
PR in draft.** This is an export-domain/copy slice, not completion of Task 12.

Rebased base: `5fd74f06fe4031b37bf84822d218943ec9ed970d` (Task 6 merged).
One dedicated PR targets `rewrite-in-rust-native-desktop`. Linux x86-64 and
macOS ARM/Intel are in scope; Windows remains deferred. Each numbered subtask
has its own Conventional Commit with an `OpenSpec-Task` trailer; review fixes
are separate commits. This is headless implementation, not native UI or package
qualification. The canonical OpenSpec repository remains local and unpublished.

## 12.1 — Explicit scopes

`export::Scope` separates current query, numeric selection, dynamic Collections,
approved Shelf union, refined Shelf and Shelf UUID selection. Resolution runs
inside one existing read-worker transaction; query pages and import limits do
not restrict it. UUID deduplication preserves first-source order, Shelf order
and unavailable cached titles, with overlap counts. Sources and pending review
proposals never replace approved membership. Invalid Collection criteria refuse
the whole export. Raw Collection names retain their bytes in the source snapshot.

The resolved row/string/dedup accounting has an explicit 64 MiB refusal rather
than silent truncation. Temporary approved-member maps have the same separate
budget; this is not a process-wide allocator or graceful-OOM guarantee. Existing
Shelf APIs still own their returned membership vectors. The 1,001-story real
SQLite test covers selection order, both unions, unavailable selection, view
refinement and page independence. Parity: P43–P48, P55; P60 remains dependent on
the complete import/runtime and packaged workflow gates.

## 12.2 — Preflight and selected destination

Preflight verifies recorded compatibility, original suffix/name, actual retained
managed source size/hash, and destination collisions. Nested legacy archive
parents are retained and opened without following links. File reads run outside
SQLite transactions. Broken/unavailable/invalid rows do not reserve filenames
or eligible bytes. Collision keys reuse frozen Unicode simple lowercase (including
`İ`), not the host Unicode version. Destination selection retains the directory
object and rechecks its path identity; a create-new/flush/owned-cleanup probe
establishes writability. Library-internal destinations refuse. Partial creation
or cleanup failures retain the actual file/directory in `Exports` and block intake.
Opaque process-local IDs identify registry-owned plans; cloned preview DTOs confer
no copy authority. Tests cover mixed rows, exact bytes, nested archives, Unicode
collisions, existing output preservation and cancellation. Frozen codec provenance
is refreshed only for the additive simple-lowercase wrapper; source/data pins stay
unchanged.

## 12.3 — Preparation and queued eligibility

The registry retains at most eight plans and 64 MiB of conservatively accounted
row/preview data, evicting oldest plans when necessary. IDs belong to one registry
and cannot be replayed or moved to another. `start` consumes the ID, revalidates
the selected directory, and compares every UUID/record/Broken state in one read
transaction before returning a running owner. The running owner borrows the
actual registry and Database; it cannot outlive them or race another run through
that registry. Per-item checks require the original record, archive metadata and
eligibility immediately before copying; prepared skipped/conflicted rows never
promote. `invalidate` discards preparations before a future library transition.
Tests cover eviction, forged/foreign/replayed IDs, stale flags, original skips,
and a queued ready item becoming Broken. Scheduling/terminal persistence remains
the separately incomplete Task 6 composition rather than an inferred job port.

## 12.4 — Exact destination-local passthrough

The running owner copies one queued ready item at a time, outside SQL, through
a create-new destination-local temporary with 64 KiB buffers and cooperative
cancellation. It compares the source stream and read-back output hash/size,
revalidates retained source/parent/name witnesses and requests file/device flush
before atomic `NOREPLACE` publication and destination-directory durability.
An existing final name is never replaced. Non-collision publication failures or
lost post-publication acknowledgements retain the real file/directory plus both
attempted names and prevent more copying; they are not reported as proven failure
or compensated through guessed paths. Successful rows return stable copy data;
the Task 6 operation owner still supplies durable reports and scheduling.

The seven admitted synthetic format fixtures (plain/v1/v2/generic PK, ZIP,
STUdio ZIP and 7z) export with exact original names and bytes and no extra output
files. The corpus/preservation hashes remain unchanged. Some suffix variants
share identical ZIP bytes, so tests use independent libraries rather than
violating unique-checksum import identity. This proves passthrough preservation
of Go-qualified synthetic input, not a new archive-inspection port or an actual
Lunii.QT GUI execution.

## 12.5 — Native durable copy, cancellation and restart

`Exports::run_prepared` consumes the actual registry-owned preparation, validates
its scope, and hosts a Task 6 worker session borrowing the admitted native
Database and LibraryFiles. The event loop receives one move-only validated Start.
Workers retain actual source/parent descriptors, custody and the registered
private destination workspace through compensation, SQL reporting and settlement.
The selected destination object is checked against the acquired actual parent.
No existing manual copy/preview DTO gains restart cleanup authority.

The registered publisher validates the saved output name, uses no-clobber rename
and flushes both directory entries. After publication, cancellation cannot erase
the completed result. Uncertain publication/durability retains its actual owner
and reconciles uncancelled; cleanup never receives a final name. Before-commit
failures and cancellation retire only the owned private workspace before reporting.
Profile/queue refusal before acquisition is retryable; read refusal is not treated
as EOF or a completed hash. Existing registry-local copies remain available for
the isolated passthrough slice.

Tests cover fragmented writes, an injected ENOSPC prefix, refused source reads,
chunk-boundary cancellation, a real native export with durable History and a
raced destination, and cancellation during a second native copy while preserving
the first completed file and source bytes. The sixteen-transition kill/restart
matrix now calls the actual registered publisher rather than a fixture rename.
It preserves completed exports and removes registered partials; link substitutions
remain refused. Injected disk/read failures are adapter-boundary evidence, not
qualification of every full/removable filesystem or Windows sharing mode.

## Remaining dependency and acceptance gates

12.8 diagnostics presentation/documentation and 12.9 final acceptance remain incomplete. 12.9's complete import→inspect→export→reopen
slice still depends on Task 5's real hostile archive inspection/import custody,
which remains incomplete. Existing preserved Go fixtures do not establish that
missing Rust import port. Keep that acceptance subtask open until it can run.

## Independent partial-slice review corrections

The Standards review of `8c7935f` found two P2 issues: a registry-local uncertainty
fence could be bypassed by a replacement registry, and preparation accounting
omitted source snapshots. The Spec review found the same accounting issue plus
Unicode suffix drift and whole-preview failure on unsafe source links.

Separate fix commits retain frozen Unicode suffix semantics, report file/parent
link refusals per item, account source IDs/raw names/vector/preview/path capacities,
and funnel every actual output retention into a sticky Database/native generation
fence. The generation continues joining its real workers on close but cannot
acknowledge clean closure with an unresolved external effect. Partial acquisition,
cleanup flush and post-publication flush tests verify actual retained file/directory
ownership; replacing or dropping the registry cannot clear the fence. Regression
tests were observed failing before their respective semantic fixes and passing
afterwards. The first cumulative gate also caught two narrow Clippy layout/style
corrections, committed separately from initial subtasks. Full final-head checks
and provider results are recorded on the draft PR; this review does not satisfy
the still-open 12.9 acceptance gate.

## 12.5 — Native integration review corrections

Independent Standards and Spec review identified loss of non-UTF-8 Linux paths;
the Spec axis also identified fenced partial-acquisition compensation and
preflight skips being rejected during durable job creation. A separate fix carries
exact native destination bytes through immutable SQL scopes and control records
while retaining legacy UTF-8 encodings. Collection display names use Go JSON
replacement semantics; numeric Collection identities and raw preparation names
remain intact. Initial skipped/conflicted rows are persisted atomically with
completed counts; ready rows still require the full copy-field validation.

Actual retained export allocations can reconcile only control records matching
their own immutable registration and previously retained control-file witnesses,
then complete required cleanup through a fenced intake. This does not clear the
profile fence or authorize a clean native shutdown. Tests cover both an injected
post-acquisition durability failure and a lost control-record acknowledgement,
terminal reporting, explicit restart, raw Collection names and mixed unsafe skips.
Native-path equality retains spelling and bytes. Actual non-UTF-8 filesystem
cases run on Linux; APFS on the development Mac refuses those components.

The follow-up review caught a retirement retry gap after payload/workspace removal
and before directory/control acknowledgement. Cleanup now observes the retained
unlinked file and records actual workspace retirement, retries durability and
control retirement, and refuses replacement names. The regression includes both
partial cleanup boundaries and a lost forget acknowledgement, then repeats cleanup
to establish idempotence. Go display-name replacement also consumes each invalid
UTF-8 byte separately, including malformed multibyte prefixes.

## 12.6 — Durable removal and restart rollback

The blocking `Removals` service borrows the actual Database and LibraryFiles,
retaining the same global custody guard, no-follow managed/trash parent chain and
archive descriptor through intent persistence, no-clobber move and guarded story
deletion. The original source file remains untouched; SQL deletion cascades tag
assignments and the intent, while Shelf UUID decisions survive. Removal uses
migration 003 unchanged and adds no operation kind or in-app restore.

Cancellation and SQL failure trigger uncancelled restoration before intent
retirement. A fresh writer barrier distinguishes a committed deletion from an
unacknowledged transaction. Failed repair retains the actual owner and fences
ordinary intake; repair can finish through that fence, without authorizing clean
closure of the uncertain generation. Startup reconciles one bounded coherent
intent at a time before returning a database; ambiguous custody refuses startup.
Native closure permits retained trash and requires a drained removal-intent table.

Tests cover intent/move cancellation through stopped intake, a refused deletion,
exact retained trash bytes, tag cascade, original source preservation, Shelf
membership, a foreign restore collision and retry with the same actual owner.
Three real child-process kills exercise intent, trash move and committed deletion;
reopening restores unfinished removals and retains completed trash. Both prepared
and queued exports reject a story with an active removal intent.

The independent removal review identified an unintended checksum eligibility gate
and its uncancelled pre-move hashing. A separate correction removes bulk hashing
from removal: no-follow parent/name and actual regular-file descriptor identity
provide custody; imported checksums remain immutable SQL scope, not removal
eligibility. Changed-size, validation-invalid archives can be moved to trash or
restored after interruption. The expanded matrix includes a corrupt-file kill
following its actual trash move. No archive data is read in a removal SQL callback.

## 12.7 — Bounded logs and explicit diagnostics ZIP

One logger per actual Database generation borrows LibraryFiles and its retained
logs directory. Typed event/level/state inputs exclude free-form paths, titles,
tokens and error strings. A serialized ring retains five fixed regular files of
at most 256 KiB each, flushing each append and rotation. Replacements, symlinks
and hard links refuse; partial writes/rotation retain actual file owners and
fence clean closure. Read-only snapshot refusal does not claim a library effect.

The explicit diagnostics service accepts a selected external directory, snapshots
only the five known log names, strictly parses/re-encodes allowed fields, and
builds a bounded stored ZIP containing `diagnostics.json` plus sanitized logs.
The manifest contains format/schema versions, generation time, OS/architecture
and aggregate story/Shelf/operation counts. It contains no IDs, paths, titles,
SQLite pages, archive bytes or tokens. Publication uses the same destination-local
stream/hash/readback/flush/no-clobber mechanism as exports, retaining actual
outputs on uncertainty. No network transport is involved. Logger record/flush and
service calls are blocking-worker APIs; native UI lifecycle wiring remains Task 15.

Tests rotate a full five-file ring across six actual boundaries, check the output
with Python's independent ZIP/CRC reader, and inject unknown fields, tokens, paths,
invalid states and malformed/trailing JSON. Raced files survive, cancellation
cleans only the owned temporary, and lost publication durability retains the
actual final output and fences clean closure. The small stored ZIP writer follows
[PKWARE APPNOTE 6.3.10](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT),
sections 4.3.7, 4.3.12 and 4.3.16; it adds no archive-reader/decompression surface.
