# Task 5: archive inspection and immutable import custody

Dedicated branch: `codex/rewrite-05-import`, based on epic
`5fd74f06fe4031b37bf84822d218943ec9ed970d`. The PR targets
`rewrite-in-rust-native-desktop`. Each subtask has its own commit. Linux x86-64
and macOS ARM/Intel are required; Windows remains deferred.

## 5.1 — Actual container dispatch

The adapter opens the supplied retained seekable source, builds an index through
existing core inventory policy, and dispatches from the original filename plus
actual regular root entries. ZIP/7z mismatches refuse. STUdio root content takes
precedence and is rejected with every `.pk` variant. Plain content takes
precedence over named v1/v2 packs. STUdio ZIP and STUdio 7z retain distinct labels.
An opened index is not a successful inspection or custody receipt.

Candidate readers are pinned to rawzip 0.5.2 (MIT), flate2 1.1.10
(MIT OR Apache-2.0; already present transitively), and sevenz-turbo 0.29.2
(Apache-2.0). rawzip streams the central directory into a fixed 256 KiB buffer,
so the application checks entries before retaining a complete index. The 7z
candidate is a maintained in-process fork with constructor-level header, count,
name, coder and memory controls. Defaults, encryption, compression-writing and
extraction utility features are disabled; optional baseline decoder codecs are
explicit. It does not invoke a user-installed helper or introduce a process.

The prior sevenz-rust2 0.24.1 experiment in PR #38 remains unqualified and is
not imported. The newer candidate's API is a reason to test it, not evidence of
complete qualification. Current provisional ceilings are 16 MiB encoded/decoded
headers and 256 MiB decoder state, in addition to the unchanged core limits.
These and upstream structural ceilings require individual hardening records,
retained counterexamples and independent review before import admission. Full
codec/solid/CRC/cancellation/resource qualification remains 5.2/5.9/5.10 work.

Evidence: four adapter tests over unchanged preservation archives cover the
accepted suffix/content matrix, STUdio `.pk` rejection, wrong container/suffix,
selected reads and actual metadata-byte overflow. Focused `cargo test --locked
-p librairii-adapters archives::` passed 4 tests (10 unrelated filtered);
all-target adapter Clippy with `-D warnings` and rustfmt pass. These are host
checks, not three-target qualification or completion of the whole import task.

## 5.2 — Bounded index and reads

Every observed entry goes through the frozen InventoryBuilder and unsigned-to-i64
size conversion before retention. This preserves path cleaning solely as an index
key, distinct raw names, duplicate normalized-key refusal, directory/link rules,
nested archive suffixes, overflow-safe counts/sizes and each family's existing
ratio denominator. ZIP symlinks and 7z special/reparse/anti entries cannot become
regular assets. ZIP encryption flags cannot enable implicit decryption. Caller
limits can reduce the frozen limits for testing but cannot increase them.

The 7z constructor receives limits before parsing and maps count/name/output
ceilings to typed validation codes. Actual selected reads use fixed chunks plus
one detection byte, fallible output reservations, cancellation between chunks and
source reads, and never return partial metadata on a read/CRC/size failure.
Read/seek cancellation uses a non-retryable I/O failure internally; using
Interrupted would make read_exact retry cancellation indefinitely.

Eight focused adapter tests pass, covering actual frozen hostile archives, raw
path aliases/invalid UTF-8/traversal/drives/backslashes/depth/length, duplicate
keys, nested suffixes, 7z links/ratios, lowered entry/expanded limits, exact actual
read boundaries and corrupt ZIP payload CRC. Test writers explicitly use verbatim
paths so their normalization cannot hide hostile names. fmt and all-target
adapter Clippy pass. New decoder constructor hardening/codec/resource evidence
still belongs to 5.9/5.10; these index tests alone do not qualify the reader.

## 5.3 — Actual pack identity

Pack identity validation now consumes the observed index and bounded uuid.bin
bytes. Plain files require their complete fixed entry set plus rf/sf children;
UUID-root packs require one distinct raw root with ni/li/ri/si and rf/sf children.
The import-local UUID parser preserves legacy accepted spellings and nil
rejection. Filenames never supply a missing identity. This phase alone does not
establish full payload integrity or import publication.

Eleven focused archive tests pass, including all six preserved valid plain/v1/v2/
generic/ZIP/7z pack fixtures, actual short/long/nil UUID payloads, plain-content
precedence over v2 suffix, missing required entries and two raw roots encoding the
same canonical UUID. fmt and all-target adapter Clippy pass.

## 5.4 — Actual STUdio metadata and thumbnail

Bounded story.json bytes now feed the preserved fixed-shape Go-compatible JSON
policy: complete-first-value syntax, typed fields, duplicate/null semantics,
trailing input, UUID, ordered stage references and exact asset lookup. Stage
references are checked against regular observed entries without extracting media.
The required thumbnail is actually read with the 10 MiB cap and parsed as PNG,
with dimensions/allocation limits passed before decoder construction. ZIP and
7z keep their distinct format labels. JSON projection returns authored text
trimmed by the frozen whitespace rules.

Fourteen focused archive tests pass. Both preserved STUdio fixtures match UUID,
title, description and exact thumbnail bytes. Actual archives cover type/trailing
JSON, invalid UUID, empty/missing/unsafe/aliased references, missing thumbnails and
non-PNG thumbnails. fmt and all-target adapter Clippy pass. Image hardening
counterexamples and their explicit compatibility record are part of 5.5.

## 5.5 — Embedded metadata and bounded PNG configuration

The actual reader selects only regular root _metadata.json/_thumbnail.png for
packs. Optional metadata retains strict Go-compatible string/null/duplicate/
trailing behavior and canonical UUID consistency without replacing the primary
identity. STUdio uses its own mandatory metadata/artwork branch. Empty authored
fields remain empty; the existing query resolver separately applies official,
embedded and `Story <UUID>`/placeholder fallback provenance. Import does not
invent a filename title or overwrite official fields.

PNG configuration receives the existing thumbnail pipeline's limits before
construction: dimension 8192, 16 Mi pixels, 128 MiB decoder allocations, in
addition to the frozen 10 MiB compressed-artwork ceiling. Configuration parsing
preserves the baseline header-validation policy; no full raster decode is added.
The allocation bound includes the PNG header/text metadata reader. As documented
upstream, decoder allocation accounting does not include arbitrary allocator
slack or a process-wide memory guarantee.

`crates/librairii-adapters/testdata/import-hardening/` retains original valid PNG
archives exceeding dimension and pixel limits, deterministic streaming generator,
byte/hash manifest, exact probe source and actual Go 1.26.5 observations. Both Go
inspections returned studio_zip and the expected UUID without errors; Rust returns
artwork_limit. These are explicit compatibility changes with rationale and remain
pending independent 5.10 review. Reproduce the Go probe by copying probe.go.txt
into target/task5-go-probe/main.go and running the recorded command from the root.

Eighteen focused archive tests pass. The entire unchanged 20-case preserved
inspection corpus matches format/UUID/text/artwork SHA-256 or rejection code.
Additional tests cover actual optional metadata absence/null/empty/type/trailing/
UUID conflicts and independent artwork, plus both retained image hardening cases.
fmt and all-target adapter Clippy pass. Publication/reopen provenance acceptance
remains part of 5.7/5.10.

## Archive dependency policy follow-up

The first full gate correctly rejected newly encountered Zlib and bzip2-1.0.6
terms plus two incompatible RustCrypto API versions. Review of the actual pinned
notices permits redistribution with retained origin/notices, changed-source and
non-endorsement conditions; both licenses join the explicit allowlist. The
embedded Mark Adler Zlib notice in crc-turbo's combine.rs is retained separately
under .config/upstream-notices/crc-turbo-1.11.0 and collected by the existing
inventory route. No upstream implementation source is vendored.

Only digest 0.10.7 and crypto-common 0.1.7 receive exact duplicate exceptions
expiring 2027-01-22. They are required by pinned crc-turbo versus the existing
sha2 0.11 API. No advisory or dependency subtree is ignored. The dependency
inventory and generated notices cover all three target graphs.

The two Brotli allocator packages omit BSD notice files from their registry
archives. Supplemental notices were retrieved from their official Dropbox
repository at immutable revision 0a81fd6928ea3b33c8cd484aa4575d50ffb98012; exact
URLs/hashes are retained alongside each notice. The full `cargo xtask check`
then passed on aarch64-apple-darwin, including both dependency graphs, notice
inventory, source/preservation checks, locked build, fmt, Clippy, nextest and
separate doctests. This intermediate gate does not complete import custody.

## 5.6 — Retained source and content-addressed publication

PickedSource is explicit read-only admission, separate from managed-file mutation
or cleanup authority. It retains the opened regular source FD and resolved parent,
keeps the selected original native filename, and observes size/identity/link/time
changes before and after streaming. Selected symlink/hardlink aliases are readable
as in Go; they grant no source write or delete capability. Cooperative metadata
checks do not claim exclusion of a hostile in-place writer.

One 64 KiB buffer copies into the actual private OwnedStage while computing
SHA-256; a second same-FD staging measurement must agree. Source read/copy errors
and cancellation leave the actual stage with its caller. Publication retains a
hash-typed directory route from the same library/DB/custody owner, flushes the
parent/child and uses the existing no-clobber/hash-verifying publisher at
archives/<SHA-256>/<original native filename>. Exact crash-left files are reusable;
foreign bytes are never replaced. Private stage retirement still uses the actual
owner and verified directory entries, never a reconstructed pathname. Backslash
managed names retain the legacy SafeJoin refusal.

Three native custody regressions pass: chunk-boundary streaming/hash/publication/
exact retry, read-only alias/substitution refusal, and cancelled-copy retirement
with unchanged source and no managed publication. Unicode/spaces/case are tested
on macOS/Linux; Linux additionally exercises raw invalid-UTF-8 filename bytes
(the native macOS filesystem refuses creation of that name). Adapter and storage
Clippy/fmt pass. Native operation registration, SQL commitment, compensation and
restart acceptance are completed by 5.7/5.8.

## Runtime integration correction

The first Linux focused CI run (39757124135, source 3e43445) reproduced the
pre-existing shutdown/finish-scan race: item cancellation was durable while the
parent remained Running after drain. The isolated fix and deterministic scan gate
are carried from pending Task 12 commit 8a116c312caa7d323932f8a3b3b9f3c15e506199,
without its export changes. Shutdown now performs a fresh same-writer finishing
scan after all workers release their owners. Six runtime failure tests pass,
including the forced ordering that exposes the original race. Initial native
Linux/ARM Mac/Intel Mac checks passed; the focused failure is not treated as a
pass or hidden by rerunning unchanged tests.

## 5.7 — Atomic catalog publication and native composition

The native ImportExecutor binds explicit picker PathBufs to move-only prepared
commands and opaque item slots. Full personal paths never become durable item
fields or cleanup authority. Preparation retains at most eight batches/16 MiB,
opens source FDs only transiently, returns the original preparation on intake
refusal, and prunes terminal bindings. Work runs on the existing bounded native
job workers with the real ArchiveInspector and ImportStorage. This is headless
composition; the desktop executable still awaits its qualified GUI.

Each native stage registers its actual allocation against the real operation/item
scope before payload writes. Copy/inspection release global custody while private
owners retain their DB/library loans. A fresh writer-barrier checksum/UUID check
under reacquired custody precedes publication. Exact duplicates skip inspection;
UUID conflicts publish nothing. Story, archive, normalized display name and derived
Age hook commit atomically in the existing schema. The inspector verifies all ZIP
regular payload sizes/CRCs; 7z keeps selected-entry semantics. Storage cancellation
observes the actual native job flag before the transaction commit boundary.

Archive/artwork receipts and custody remain live through durable item reporting.
Known SQL rollback uses a new same-writer absence observation bound to the exact
managed paths and actual retained guard, together with the original publication
owners. Only newly owned files move back into retained staging then recoverable
trash; reused existing bytes are preserved. Unknown writes are not repeated:
exact catalog readback precedes any compensation decision. Uncertain filesystem
owners remain retained/fenced for recovery, and dropping an unsettled import owner
cannot certify clean shutdown. Settlement validates its actual correlated durable
report; post-commit cancellation cannot replace Imported with Cancelled.

Original filename and managed-path SQL TEXT preserve raw native bytes. Query
snapshots retain those bytes separately; unrepresentable UTF-8 presentation fields
are empty instead of becoming renamed paths. Follow-on export composition must
consume the raw fields or refuse representation. Normal UTF-8 JSON envelopes and
all frozen serialized fields remain unchanged. The existing official/embedded/
fallback resolver and UUID-based Shelf reconnection remain authoritative.

Three real executor integration tests pass: concurrent exact imports and UUID
conflict with one catalog/managed archive; actual SQL-trigger failure with atomic
rollback and owned archive/artwork trash compensation; and strict archive failures
with frozen persisted reason/message identities. The first test closes/reopens the
same SQLite and checks source/managed bytes and provenance. Full workspace
all-target Clippy and fmt pass. Native registered-stage/restart/failure matrices
and complete codec/hardening review remain 5.8–5.10 gates.

## 5.8 — Effect failures, cancellation and process death

Nine executor integration tests pass. The failure matrix refuses source reads,
copy/hash, inspection, archive/artwork publication, SQL and settlement at their
actual boundaries. A private staging corruption fails the independent SHA-256
measurement; unavailable picker inputs become per-item failures. A child with a
verified POSIX file-size limit performs a real short write after 8 MiB + 17 bytes;
the exact partial private copy remains recoverable and its source is unchanged.
Actual SQLite trigger failure rolls back all catalog changes before compensation.

Cancellation uses the real native job flag during copy and immediately before
catalog admission; cancellation after the actual SQL commit preserves Imported.
Forced child-process death during copy, after archive publication and after SQL
commit leaves registered original private stages for native startup reconciliation.
Recovery interrupts history, cleans only those registered allocations, preserves
committed managed bytes and supports explicit re-selection: pre-commit retry
imports/reuses exact orphan bytes; post-commit retry reports DuplicateChecksum.
A separate real native import closes and reopens the library with its new row,
metadata and unchanged legacy database and source archive.

These scenarios exposed two integration corrections. Bounded native ownership
observations serialize across file/DB workers so read-only checks do not refuse
each other as a concurrent profile update. Real external profile updates retain
their existing admission/refusal semantics. Four concurrent observers exercise
400 successful ownership checks. After durable duplicate acknowledgement only,
the original unused private stage can be discarded under its retained custody;
unknown unlink/flush results retain the actual owner and fence intake. Failure
compensation still preserves recoverable trash. General trash settlement/adoption
release qualification remains a separate release gate; these tests do not certify
arbitrary crash leftovers or substituted allocations as safe to remove.

The independent standards review found a custody-loss path before artwork file
setup: taking the guard before a fallible directory open could leave compensation
without its original guard. Directory preparation now finishes while resources
retain custody. The generalized artwork route names also describe both embedded
and official categories. These are separate review fixes, not waived findings.

## 5.9 — In progress: bounded fuzzing and complete codec parity

The new retained synthetic corpus exercises Copy, LZMA/LZMA2, PPMd, Deflate,
BZip2, independent Zstd/Brotli/LZ4 streams and BCJ/BCJ2/PPC/ARM/ARM64/SPARC/Delta
filters through actual Studio inspection. UUID, projected text and exact artwork
bytes match. ZIP fully validates an unused corrupt payload's CRC, whereas 7z
accepts a corrupt unrelated block without decoding it. Selected 7z reads now use
the actual block map and original constructor limits; global all-block traversal
cannot honor an early-stop callback between phases. Earlier solid members drain
through32KiB cancellation checkpoints. Repeated selected reads may decode the
same solid prefix repeatedly; the preserved2GiB logical expansion cap is not a
single cumulative2GiB inspection-work claim.

The source reader has a bounded encoded-header method preflight to preserve
Go's decoder set before header decoding, as well as the selected-block method
gate. IA64/Thumb/RISC-V filters remain unsupported where Go rejects them. The
preflight only observes methods; the candidate still validates the full container,
CRC, stream graph and allocation bounds. UTF16 total-name accounting includes
all NUL terminators and the external flag. Production allocation ceilings are
16MiB stored/decoded headers and256MiB decoder estimates; reduced public budgets
cannot raise them. PNG config parsing bounds dimensions/pixels and128MiB parser
allocation. The pinned png decoder's iCCP inflate is capped by its remaining
byte budget and invalid/oversized optional profiles are discarded; config parsing
does not rasterize image data.

Individual retained previously accepted Go counterexamples cover dimensions,
pixels, incomplete PNG framing, selected/predecessor7z CRC,
end/decoded-header buffering, decoder memory, per-block coder/stream counts,
aggregate coders and unmodelled unused decoder memory. Their hashes, observed Go
results, rationales and required Rust outcomes remain separate hardening records
pending independent5.10 acceptance. The frozen preservation corpus is unchanged.

Both pinned bounded fuzz targets pass after the header preflight change:
identity11140824cases and import_boundaries607849cases in31seconds each. Import
fuzzing uses actual ZIP/7z/path/Studio/optional JSON decoders,64entries,128KiB
expansion,16KiB input/header/metadata/artwork and8MiB decoder memory, with a2second
per-input timeout and512MiB process RSS ceiling. Retained seeds and regressions
are committed; generated corpus and crash artifacts remain task-local.

**Qualification blocker:** encoded-header-bcj2.7z is valid and accepted by the
frozen Go inspector, but sevenz-turbo0.29.2's encoded-header path refuses multi-I/O
coders with `Multi input/output stream coders are not yet supported`. Its body
BCJ2 decoder succeeds, so a body-only codec matrix would conceal the gap. The
required Rust result remains Accepted and the parity test deliberately fails.
This is lost supported behavior, not an intentional hardening exception. Task5.9
and5.10 remain unchecked; the dependency is not qualified. The user was asked to
choose a pinned, reviewed decoder repair or a replacement candidate before that
design-dependent work proceeds.

Independent spec review additionally found that the candidate's fixed32MiB
Brotli estimate does not bound its allocator: locked brotli-decompressor6.0.1's
custom-dictionary state enables large-window mode, which can allocate a1GiB ring
buffer for a non-last metablock before a small output/header cap intervenes.
No hostile allocation probe was run. The Brotli feature is disabled in both
locked dependency graphs pending a bounded implementation. Its retained normal
Go-accepted fixture still requires Accepted in the failing parity manifest;
this temporary safety refusal is not a compatibility waiver. The ordinary
positive codec test now exercises the15 remaining body codecs/filters.

Two more qualification questions remain explicit: the parser's10000 structural
count ceiling also applies to packed streams, so a separate >10000-stream/
<=10000-file counterexample is required; and the Zstd decoder's additional128MiB
window clamp needs its own comparison/record rather than assuming the general
256MiB memory record covers it. These gaps and the safety defect keep5.9/5.10
open even if a particular normal codec corpus passes.
