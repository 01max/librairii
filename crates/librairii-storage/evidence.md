# Section 4 storage compatibility evidence

Scope: generic storage only. Import and operation/history domain repositories,
their reserved tests, root graph/shared ports and Go bridge17 are excluded.
Release: `ROOT4-4ea2caf`; approved base
`4ea2caf71bbe61ed996942daa9005de05b75f9da` (parents9a+reviewede60;
tree `de6034c706dac0c5e8a1f3aa68a8a9fd565015fd`). Root's release activates
owner5's prepared initial storage-shell handoff. ActualM provider checks were
pending at implementation start; owner2 subsequently confirmed all actualM CI
green. This is foundation evidence, not native storage qualification.

Implementation proceeds in numbered order with a dedicated Conventional Commit
and `OpenSpec-Task: 4.N` trailer for each subtask; fixes remain separate. Exact
completed SHAs are added in a following commit, so no commit claims its own SHA.
Owner2 alone updates canonical OpenSpec checkboxes. This ledger is shipping
evidence, not an additional checklist or acceptance of unfinished work.

| Task | Implementation/verification work | State / commit |
| --- | --- | --- |
|4.1|Explicit fresh isolated root; exact layout, managed paths, raw identity; reject escape/invalid/nonfresh inputs without profile discovery|Implemented/host verified `9c37f56ffe83d73497129603c4ecfb8d25e4e087`. Setup `fcf467f41a39c3670a38db063a5304a60f6e93d1`|
|4.2|Exact migrations001-018, schema identity and contiguous ledger|Implemented/host verified `ea8e60f415b5ea23fb50f8899594884ef13b6c8f`; probe hardening `bbf386bd4e681d3a7732fae28af750f173da6211`. Setup `e6e527af79a21149665670a36171ef5d701be05c`; owner5 graph `bf720718c4736e0ecf5f6ded947ccbce49f1ba16`|
|4.3|One owned writer plus four reads, bounded jobs/transactions and close|Implemented/host verified `8baebc751ed51e917a3ae5b139ebf48fc5f0467b`|
|4.4|All18 admitted populated prefixes, full schema/rows/query outcomes and actual Collection-open projection; no fixture-specific patches or eager DB-open rewrite|Pending real canonicalization dependency; placement accepted2/5, no later-feature deferral|
|4.5|Online WAL backup, integrity/FK, rollback and foreign DB+sidecars preservation|Implemented/host verified `abeb80ecd401f840e45a6a162998f5808cd9ebc1` on Unix; Windows directory durability remains pending4.9|
|4.6|Strict64KiB v1 profiles, atomic serialized updates and isolated native namespace|Implemented/host verified; source SHA recorded in following commit. Stronger race capabilities and Windows effects remain4.8/4.9|
|4.7|OS lease through close, refuse unproven legacy exclusion, verified independent adoption|Pending|
|4.8|Distinct no-clobber/config-replace/durable-flush effects and Unix races|Pending|
|4.9|Windows effects/path/share-mode implementation plus actual-target evidence|Pending|
|4.10|Portability preflight and identity-verified rebinding without foreign cleanup|Pending|
|4.11|Independent exact-head AO review, schema018 Go→Rust reopen/archive hashes, all cumulative/provider checks|Open gate|

Fixture authority: admitted146-file PR39 corpus at
`1cd22443cae3b548bb7f9b3fb2435dcd73c3c65c`, manifest SHA256
`a59f2194cf5c1ad317485a8714e703c12fe161bfad0190a67ba5816ea7f46e53`;
database provenance `3cc94f961594c2739a1ab6d5f81c2abedf95d5d374853f3121ca7adf0c42f9b0`.
All19 source DBs/18 SQL files and original archives remain read-only inputs.
Frozen Go oracle: `73a93303a9a44cf0a92eaf525331561ed8c04460`.

Required verification is cumulative: retained Rust nextest+doctests and
preservation assertions, all Go/React checks, then exact-head native/provider
checks. Host-only/cross-build results never qualify other native filesystems.
Unavailable required evidence stays pending; tests use only TestRoots-owned
fresh directories or independently materialized verified copies.

4.1 graph support: owner5's `f2cea76445a707a348d433f61f7e5836c919541f`
is a direct descendant of the manifest setup and changes only three Cargo.lock
edges. Both commits are retained by fast-forward. Root Cargo/lock were not
authored by owner6. Pinned nextest0.9.148 is reused from the section3 tool root.

The first host test incorrectly assumed macOS could materialize non-UTF-8
names. The managed sandbox returned EPERM; the same isolated test outside it
returned EILSEQ (92). The target-specific contract now asserts lossless real
materialization on Linux and non-destructive refusal/no lossy replacement on
macOS. No frozen assertion was changed. Matching native Linux/Windows evidence
remains a separate requirement; no host-only portability qualification follows.

4.1 host verification (aarch64-apple-darwin, stable1.99.1):

- `cargo nextest run -p librairii-storage --test layout_identity --locked`:
  11 passed, zero skipped, run144a4e55-9977-4c75-a946-dab3d0a13234.
- `cargo clippy -p librairii-storage --all-targets --locked -- -D warnings`
  and `cargo fmt -p librairii-storage -- --check`: passed.
- `cargo nextest run --workspace --locked`:35 passed, zero skipped,
  runfa7729d6-bf80-497d-b5cb-c9f5631b0624. The first sandboxed run had34 pass
  and EPERM at the existing loopback-binding test; the unchanged full suite
  passed outside that network sandbox. No assertion or required test was skipped.
- `cargo test --workspace --doc --locked`: both compile-fail doctests passed.

These are headless host checks, not Intel/Linux/Windows filesystem qualification
or final4.11 review. Full retained Go/React and final cumulative quality/provider
checks remain required before section4 admission.

4.2 host verification uses the separately resolved exact rusqlite0.40.2
(`default-features=false`, `bundled,backup`) graph supplied by owner5. The
root manifest, lockfile and additive4.1 discovery support in `bf720718` were
consumed by fast-forward; owner6 authored none of those shared files.

- Four migration tests passed, zero skipped, nextest run
  `552a3d94-d1f9-4b64-bd29-5bf51475c61a`: exact18-file byte equality against
  admitted corpus and frozen Go SQL; all18 real prefix identities/ledgers/full
  schema structures; missing-trigger/schema forgery; foreign/empty/gapped/
  duplicate/unsorted/future ledgers. All original DB bytes remained unchanged.
- Closed admitted standalone DBs are inspected with `immutable=1`, not a live
  WAL copy. The recognizer warns that arbitrary read-only WAL opens can touch
  sidecars; production foreign-file probing remains the safe4.5 owner boundary.
- DDL comparison is exact for all non-internal tables/indexes/triggers. The
  two historical ledger whitespace spellings are accepted by equal column/PK/
  default structure without rewriting either; data and timestamps are retained.
- This step recognizes supported states; it grants no existing-library adoption
  or migration authority. Workers, backup/rollback and locks remain later tasks.

4.3 mechanisms (shared app/domain names and concrete cross-section handoffs
remain coordinated with owner5/2; no feature SQL is added here):

- Five dedicated owners create and close their connections on their own threads:
  one writer plus four read-only/query-only readers, all WAL/FK/5000ms. Each
  connection actually reports SQLite3.53.2, source ID
  `2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`.
- Each queue has sixteen ordinary waiting slots. The writer has four reserved
  report/repair slots in the SAME FIFO, so fresh barrier reads cross all earlier
  admissions. Total ordinary ownership is bounded at17 writer/20 reader results
  (including active work and unacknowledged results); repair ownership is4.
  Execution and receiver jointly retain admission. Full admission returns an
  explicit error; storage does not start blocked producers or hidden retries.
- One synchronous DB-only closure forms a transaction. Queued/precommit work
  cancellation or a body failure requires successful explicit rollback before
  reporting proven noncommit. A commit error remains uncertain. Ordinary writes
  are fenced; uncancelled repair/barrier readback remains possible. Repositories
  own their matching predicates and runtime owns retry/repair scheduling.
- Ordinary stop leaves repair usable. Close drains/joins readers, then drains
  writer/checkpoints/joins/closes before releasing the local runtime claim.
  Unexpected worker exit wakes queued receivers and permanently fences a failed
  close; it never reports unavailable effects as rollback or closed success.
  This claim is local coordination, not the later4.7 OS/legacy exclusion proof.
- Existing DBs/sidecars are refused before SQLite open by this initial API;
  compatible upgrades remain gated by4.5 backup and4.7 qualified authority.
  The immutable fixture URI helper now losslessly encodes URI punctuation and
  handles Windows separators without using lossy native-path display.

4.3 host checks: eleven real SQLite worker tests passed (run
`2abf07fa-6748-476f-afa0-833bf070cd96`); storage Clippy passed. Tests cover four
simultaneous coherent read snapshots while the separate writer commits, queue
saturation/reserved repair/FIFO barrier, unacknowledged-result capacity, queued
and precommit cancellation, late cancellation, explicit rollback, deferred-FK
commit failure, injected lost acknowledgement after real commit, readback,
dropped-result durable drain, refused foreign DB+WAL/SHM and worker-unwind
failure fencing. The acknowledgement-loss injection deliberately swallows a
real successful COMMIT inside the test callback; production callbacks forbid
transaction control. It does not claim physical I/O fault qualification.
One compile-fail doctest proves a transaction cannot escape as an owned result;
both prior compile-fail doctests remain passing. Full final cumulative/native
checks and4.11 remain open.

The final4.3 source tree passed cumulative50 nextest tests, zero skipped
(`b8a54502-8d09-4c74-ab52-cc0f1df26963`), and all three compile-fail doctests
on the host, retaining every admitted Rust test. Additive discovery pinning of
the new4.2/4.3 identities was supplied by owner5's baseline-only descendant
`deb1794d5cf8a14664697a21904d6e292acf8865`, consumed by fast-forward. Its parent
`89ea703a1e32e7833f97e7380ed32d096e70842c` moves the existing tempfile3.27.0
edge from dev to production; Cargo.lock remains byte-identical. Testkit stays
development-only. New subsequent identities still require additive owner5 pins.

Retained Go acceptance failure (not waived): PR43 head
`89ea703a1e32e7833f97e7380ed32d096e70842c`, run39463039584/job118767461252,
Linux release exited124 at17:11:35Z after frontend render17:10:40.861 and
runtime_started/ready17:10:40.865; no checkpoints.log. Artifact API returned
zero artifacts (the workflow uploads only after success; its EXIT trap removes
the temporary acceptance root). The Go/frontend/scripts/workflow/Go module
inputs have no diff from admittedM4ea to that head. This is an observed input
comparison, not a cause or baseline attribution. Owner5 leads focused triage
with2; owner6 performed read-only log/source inspection, no diagnostic rerun,
timeout/assertion change or out-of-scope source fix. Final PR43 admission stays
blocked on resolving the failure; native Rust Linux success is separate.

Owner2 subsequently verified exactly one unchanged89ea diagnostic retry:
run39463039584 attempt2/job118774649697 succeeded17:23:31Z. Original attempt1
remains failed evidence; the retry does not establish a cause or waive future
current-head checks. Owner5 retains triage ownership. No duplicate rerun or
bookkeeping-only push was performed by owner6 during that observation.

4.5 implementation and limitations:

- Existing owned-root DBs are first copied with all present WAL/SHM into a
  fresh private probe; only that independent copy is opened during schema/
  integrity validation. Symlinks and Unix hard links are refused. Stability
  stamps are rechecked but are not claimed as lifetime exclusion/file-handle
  authority; production adoption and platform race-proof ownership remain4.7/8/9.
- Probe hardening validates real identity/ledger DDL before row queries, so
  foreign views/virtual tables cannot execute as identity/history. The complete
  ledger DDL is compared ignoring only ASCII formatting whitespace, without
  rewriting stored SQL; column/PK/default structure remains verified. Identity
  reads are capped2 and ledger reads19 (first unsupported prefix), not unbounded.
- Live snapshots use SQLite's online backup from one coherent source read view,
  include WAL, compare semantic table counts, check full schema/integrity/FKs,
  close and flush standalone output. The64-page loop checks cancellation and
  monotone progress; Busy/Locked fails one attempt without hidden retry. Verified
  snapshot bytes/receipts survive flush/keep/reporting failure; uncertainty is
  explicit and does not authorize migration or destructive Drop.
- Before pending migrations, retain a durable verified pre-upgrade backup.
  Each unchanged SQL/ledger append is one transaction. Migration/post-verification
  failure closes SQLite, preserves the newer failed candidate separately, then
  restores/verifies the supported snapshot via no-clobber publication. Failures
  retain backup/candidate paths and honest restoration evidence; failed closure/
  restoration fences the runtime. Failed fresh initialization restores absence
  only after preserving its candidate; it never deletes the only failed copy.
- Normal foreign/future/gapped/corrupt open never relocates or replaces data.
  Foreign relocation is an explicit owned-root action preserving main/WAL/SHM
  in a unique recovery directory, with no automatic replacement. A partial move
  retains precise recovery paths and fences authority. Current compatible
  reopen creates no unnecessary pre-migration backup.
- On Windows, directory durability is deliberately unavailable until4.9 supplies
  and qualifies its actual capability; affected backup/upgrade/preservation
  operations refuse, with native tests for refusal. This is unfinished Windows
  implementation/evidence, not an admitted substitute or a physical qualification.

Real failure tests cover mid-backup cancellation, backup reservation and final
directory-flush failure, committed-WAL inclusion, missing/gapped/future schema,
foreign-key corruption, Unix aliases/symlinks, fresh initialization failure,
partially applied migration rollback and independent failed-candidate retention.
All source fixture/archive bytes remain unchanged. Accepted4.4 projection needs
Go Unicode15 full fold/NFKD/Mn and CLDR32 BCP47 behavior, identified in frozen
source and requested through owner5/4. Serde alone is not asserted equivalent;
no ASCII/fixture-specific query patch or eager Collection payload rewrite exists.

4.5 host validation: cumulative63 nextest tests passed with zero skips, run
`7ce78cad-a53b-4513-aefe-340a8ae52959`; all three compile-fail doctests,
storage formatting and all-target Clippy passed on aarch64-apple-darwin1.99.1.
This includes the unchanged prior assertions, strengthened existing reopen/
foreign-refusal checks, and thirteen new probe/backup/failure identities since
owner5's last additive baseline. Pinning remains through5. Current-head native
provider checks and independent4.11 review are still required; no full section4
or Windows filesystem admission follows from these host checks.

Owner5's original baseline-only descendant
`0e078249b37b9451d3f9104c455c707835b5e0fa` was consumed by fast-forward;
all13 actual backup/probe identities were added without changing earlier tests,
storage source, the root graph, lockfile or corpus. Its full ARM xtask check
passed63 nextest/zero skips and three compile-fail doctests on that committed
tree; this result is not transferred to later source changes.

4.6 implements profile data independently of library authority:

- Explicit legacy reads retain exact source bytes and raw libraryId spelling;
  unknown versions/fields, trailing values, malformed documents and documents
  over64KiB are refused without repair or fallback to a new installation. Go's
  scalar case-insensitive/last-assignment/null semantics are retained. Version1
  validates en/fr, and completed/recovering native roots require an absolute
  path and nonempty raw identity; a profile is never a library adoption proof.
- Native updates require a fresh separately marked namespace and a retained
  exclusive OS file lock using safe std APIs. Reopening checks the native
  marker; legacy directories are never silently marked or converted. Ordinary
  updates share one mutex through read/change/stage/file-sync/replace/dir-sync,
  preserving overlapping language/root changes. Settings and namespace inode
  witnesses are revalidated; changed/aliased files cannot grant replacement
  authority merely because their bytes match a prior document.
- The new document is cached only after durable replacement succeeds. A failed
  post-replace directory flush or ownership recheck returns an uncertain
  publication cause and poisons every cached load/update until the store is
  closed and reopened. No success or old-root authority is fabricated from that
  error. Staged pre-replace failure retains old bytes/cache and removes only its
  owned temporary file. Serialization preserves Go field order and HTML escapes.
- Pure explicit-context locations are macOS Preferences/io.github.librairii.native
  and Application Support/LibrairiiNative; Linux config/data librairii-native;
  Windows APPDATA/LibrairiiNative and LOCALAPPDATA/LibrairiiNative/Data. No helper
  discovers a home, environment, profile or library. Native recovery lives in
  the separate profile namespace; its path conveys no cleanup permission.
- Native namespace constructors refuse on Windows until task4.9 supplies actual
  directory durability/identity capabilities. The Windows location and refusal
  assertions are target-native definitions, not host filesystem qualification.
  Descriptor-relative ancestor-race proof and generic replace/no-clobber/flush
  integration remain4.8/4.9; no production adoption is enabled by this step.

4.6 host verification (aarch64-apple-darwin1.99.1): cumulative78 nextest tests,
zero skips, run`0a91c60d-f33e-4e1d-ace7-88ecc4cd09c0`, and all three retained
compile-fail doctests passed. Formatting and storage all-target Clippy passed.
Fifteen new actual host identities cover strict wire limits, invalid legacy and
native preservation, concurrent serialized updates, real second-process lock
exclusion/reopen, identical-byte inode and namespace substitution, alias refusal,
pre-replace staging failure and post-replace poison/restart recovery. The latter
fault seams run at real staging/replace boundaries but do not claim physical
disk-failure qualification. Additive discovery remains owner5's shared-file work.

The newly reported foundation corpus-wrapper URI defect on checkout names with
Unicode/space/#/%2F is owned solely by owner5. Ordinary-path prior successes do
not establish special-path coverage. No checker, fixture, pin or shared-tool fix
was authored here. Consume the reviewed actual integration repair by preserving
history after admission, union discovery support through5, and rerun combined
current-base checks/review before any section4 admission.
