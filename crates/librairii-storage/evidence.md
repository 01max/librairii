# Section 4 storage compatibility evidence

Scope: generic storage only. Import and operation/history domain repositories,
their reserved tests, root graph/shared ports and Go bridge17 are excluded.
Release: `ROOT4-4ea2caf`; approved base
`4ea2caf71bbe61ed996942daa9005de05b75f9da` (parents9a+reviewede60;
tree `de6034c706dac0c5e8a1f3aa68a8a9fd565015fd`). Root's release activates
owner5's prepared initial storage-shell handoff. ActualM provider checks were
pending at implementation start; owner2 subsequently confirmed all actualM CI
green. This is foundation evidence, not native storage qualification.

Each numbered subtask retains a dedicated Conventional Commit
and `OpenSpec-Task: 4.N` trailer; fixes and dependency-qualified slices remain separate. Exact
completed SHAs are added in a following commit, so no commit claims its own SHA.
Owner2 alone updates canonical OpenSpec checkboxes. This ledger is shipping
evidence, not an additional checklist or acceptance of unfinished work.

| Task | Implementation/verification work | State / commit |
| --- | --- | --- |
|4.1|Explicit fresh isolated root; exact layout, managed paths, raw identity; reject escape/invalid/nonfresh inputs without profile discovery|Implemented/host verified `9c37f56ffe83d73497129603c4ecfb8d25e4e087`. Setup `fcf467f41a39c3670a38db063a5304a60f6e93d1`|
|4.2|Exact migrations001-018, schema identity and contiguous ledger|Implemented/host verified `ea8e60f415b5ea23fb50f8899594884ef13b6c8f`; probe hardening `bbf386bd4e681d3a7732fae28af750f173da6211`. Setup `e6e527af79a21149665670a36171ef5d701be05c`; owner5 graph `bf720718c4736e0ecf5f6ded947ccbce49f1ba16`|
|4.3|One owned writer plus four reads, bounded jobs/transactions and close|Implemented/host verified `8baebc751ed51e917a3ae5b139ebf48fc5f0467b`; queue-stop failure closure correction follows, with separate poisoned-close RED/GREEN evidence|
|4.4|All18 admitted populated prefixes, full schema/rows/query outcomes and actual Collection-open projection; no fixture-specific patches or eager DB-open rewrite|Private source/normalization `a125778200af8d28e8ff99c6c41f47a6ef5b69db`, Default language `d902658ad4e8dd33dc9540658ec4176a25fe9652`, strict codec `7350ddf1a87c0434efe1e7fac49c13db9906faf2`; explicit same-writer projection `c48f7db6e891a6583c1e6b99c3d497f7f3bf9f28`, original discovery `e9c559aa35a3df5c6f2b767baa17949b2b4f3a2a`. Finite independent19-DB verification passed on exacte9; other native/current cumulative/admission gates remain open|
|4.5|Online WAL backup, integrity/FK, rollback and foreign DB+sidecars preservation|Implemented/host verified `abeb80ecd401f840e45a6a162998f5808cd9ebc1` on Unix; Windows directory durability remains pending4.9|
|4.6|Strict64KiB v1 profiles, atomic serialized updates and isolated native namespace|Implemented/host verified `2751a04a4af6b9da8a21300a182ca2dd5231d8b8`. Stronger race capabilities and Windows effects remain4.8/4.9|
|4.7|OS lease through close, refuse unproven legacy exclusion, verified independent adoption|Partial `2fb7c0126b7e1b10f46d5cd5326e6e0809b4d4cb`: fresh-root cooperative lease/refusal; closed-input verification `d8b39e0633a0d6a4a335343ce07a66a5ed20f8c4`; Unix native-state/clean restart slice `3f3591d0d7ac2db3a2a52cd5b1a1fc92fa2bb986`. Recovery/relocation and current/native qualification remain open|
|4.8|Distinct no-clobber/config-replace/durable-flush effects and Unix races|Partial `b9148a770f896445b36233452bef7a59e84a60d6`: streaming; descriptor-relative Unix publication/config/flush mechanisms host verified `d6a966e90e14bd0898b49b8469133985d6cb1fee`. Other native qualification and complete reconciliation/adoption remain pending|
|4.9|Windows effects/path/share-mode implementation plus actual-target evidence|Pending|
|4.10|Portability preflight and identity-verified rebinding without foreign cleanup|Partial foreign profile inspection/proposed document rebinding follows; filename/collation/filesystem and recovery-path qualification remain pending|
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

4.7 partial mechanism checkpoint (not completion or adoption admission):

- A fresh root acquires `.librairii-library.lock` with safe std OS file locking
  before creating identity/directories or opening SQLite. The library authority
  owns the handle beyond DB/read/report drain and closure; a runtime borrows that
  authority. Local runtime release does not prematurely release the OS lease.
  The lock file is retained on close/crash; its existence/PID is not authority.
  The filename is coordinated with5 as a future bridge seam, not an implemented
  or assumed section17 bridge.
- Runtime startup and admissions recheck the retained lock/root witnesses and
  exact original identity bytes. Substitution fences ordinary intake and new
  admissions without claiming prior work rolled back. Restoring the actual
  retained object permits critical fresh readback while ordinary intake stays
  stopped. Namespace/descriptor-race and Windows stable-ID proof remain4.8/4.9.
- `open_legacy_in_place` refuses before I/O: no currently admitted mechanism
  proves that later unmodified Go launches honor Rust/bridge locks. No PID scan,
  installation flag or user assertion can grant that missing authority. Verified
  whole-library/profile/recovery clone adoption remains required and unfinished;
  its streaming SHA256/source policy is coordinated through4/5. No custom hash,
  source alias, copied-journal cleanup or production library activation exists.
- Real child processes verify cooperative exclusion through ordinary stop,
  critical readback, DB close and library-authority drop. A separate held process
  is killed; kernel lock release permits a new owner despite the stale file.
  The fixture process is bounded and owned/joined. Changed identity is refused
  before DB creation, and changed/aliased lock names cannot refresh old authority.

Cumulative host validation passed86 nextest tests, zero skipped, run
`d0d6aaf8-55be-4f88-844a-dcaa5adda026`, plus all three compile-fail doctests,
formatting and storage all-target Clippy. Eight new source-defined lease/refusal
identities apply to the native targets indicated by their cfg (six all-target,
two Unix). These are macOS ARM observations only. Native Windows and other
filesystem qualification, legacy-launch clone isolation/restart/relocation,
all4.7 completion conditions and independent4.11 review remain open.

4.7/4.8 dependency support and actual repair integration:

- Owner6 manifest-only `ee6eba8eb86cb3750f4473feb7b988829623f730`
  introduced all-target sha2 and cfg(unix) rustix workspace edges only.
  Its native/focused provider checks failed before tests while workspace.sha2
  was absent, including run39470908137/job118794702827. This is a concrete
  composition failure, retained as failed evidence, not a baseline/environment
  issue or test qualification. No old-head retry was requested.
- Original owner5 direct descendant
  `3db1a389fd62b57d34c23eb77b17f45d865951d6` was consumed by fast-forward.
  It supplies exact sha2=0.11.1/no defaults/no extra features and Unix
  rustix=1.1.5/no defaults/fs,std, locked source/policy/inventory and24 additive
  profile/lease identities. Existing tempfile adds rustix default/alloc/std in
  the actual Unix union. There is no Windows rustix edge or Unicode product
  dependency. Owner6 authored no root manifest/lock/discovery changes.
- Actual admitted preservation-wrapper repair
  `4383373272ce1d3095199caf37dde7e5d0598213` was then merged without conflicts
  as `320c693ec76d81546d36a1670616231222ac1364`, preserving parents3db+438.
  Only the already reviewed repair's four shared files entered through that
  merge; no manual shared-file resolution or speculative base was used.

4.8 streaming measurement uses one64KiB stack buffer, checked size arithmetic,
an explicit ceiling plus at most one overflow sentinel, and SHA-256 only after
EOF. Cancellation/read errors/invalid reader counts return no partial digest.
Interrupted reads resume with cancellation checks; other I/O causes remain
available even if an independent cancellation arrives. It cannot interrupt an
arbitrary blocking Read or bound decoder allocations/preceding-solid work.
The typed Checksum/size measurement is data, not file/generation/custody,
publication, compensation or cleanup authority. No whole-file read_to_end,
source-path opening or destructive cleanup is performed by this mechanism.

The combined source tree based on320c693 plus the streaming wrapper passed
`cargo xtask check` on aarch64-apple-darwin1.99.1:92 nextest/zero skipped,
all three compile-fail doctests, read-only146-file preservation, strict Python
guard (including admitted special-checkout-path regression), root/fuzz
deny+audit, inventory/notices, locked build, formatting and Clippy. Six new
all-target source identities cover NIST empty/abc/million-a with fragmented
reads crossing65536 bytes; exact limits/overflow; mid-stream read failure;
cancel before/after a chunk; independent simultaneous error/cancel; and EINTR/
invalid reader counts. The focused run is
`da02e3aa-3ac9-46d4-a827-f995943dc148`. This was a working-tree check before
the source commit, not an exact-head provider or other-native qualification.
Owner5 owns additive pinning of the six new identities and final combined
UNION checks. All filesystem effects/adoption/4.11 admission gates remain open.

4.4 critical supplied evidence: owner2 independently read local pending
`surviving-observations.json`,734599 bytes/SHA256
`7c41d90dbaf3b9442d5c694647ba19eab494cec4b50ea43b97522bffe6dea375`.
Of160 non-Mn/nonzero-CCC sequences, both tested final pipelines differ in120;
full fold differs in0. Input a+U16FF0*29+U1715+U16FF0+Z preserves Go's
29/U1715/1 mark partition, whereas the tested composition moves all30 U16FF0
marks ahead of U1715. Both marks are Mc and survive Mn stripping. This is
observable final-key failure, not merely a raw-intermediate difference; earlier
15550/896 sample successes never established equivalence. These are supplied
local observations, not an independently rerun/read or published-source review
by owner6. No tested nfkd/stream_safe composition is implemented here. Retain
the scoped actual-Go regression after source-evidence publication and use the
accepted exact frozen algorithm/data/notices handoff; preserve all146 admitted
inputs. No fixture-specific alias, Unicode dependency substitution or semantic
waiver follows. Raw profile/UUID/path bytes remain independent of normalization.

Windows correctness checkpoint: PR43 sourceb9148a7 failed native provider run
39473603371/job118804072905 at18:36:43Z before tests: warnings-denied
`unused_mut` in layout's private directory builder and native-profile creation.
Both builders now bind immutably on Windows and shadow mutably only on Unix,
where `DirBuilderExt::mode` actually requires mutation. No lint allowance,
assertion change or unchanged-head retry is introduced. The failure is retained;
corrected-head native checks are required and not inferred from host success.
Original additive support29537950c8b5d07799fd11a76a82dfe350a83186 was consumed
by fast-forward; its six streaming identities and all prior entries are retained.


4.8 Unix mechanism checkpoint (fresh isolated authority only):

- ORIGINAL owner5 hash-discovery support29537950c8b5d07799fd11a76a82dfe350a83186
  was consumed by fast-forward. The separate Windows correctness repair is
  cab8252e24f45ac14c4974dc8382622297d38c0d. Its exact archived commit passed
  storage all-target Clippy and23 retained layout/profile tests on ARM, zero
  skipped (run194a2b83-d63a-4f58-a686-74b8fdba4066). No native Windows result
  is inferred from that check; the original b914 native failure remains above.
- LibraryFiles anchors root/staging/archive/catalog/trash directory descriptors;
  effects never reopen an ambient destination path. Original managed-directory
  inode witnesses are retained by the library authority, so reopening handles
  cannot legitimize substituted directories. Stage creation uses mkdirat and
  create-exclusive/no-follow0600 files in private0700 containers. Names remain
  lossless OS components; bounded collisions never adopt an existing object.
- All LibraryFiles instances for the SAME actual borrowed library generation
  share one move-only custody exclusion. UUID equality cannot supply another
  generation's authority. File/directory resources keep that generation/OS lease
  borrowed until closure, enforced by a compile-fail example and drop checking.
  No arbitrary path/TestRoots/serialized DTO constructs a production capability.
- Publication uses native NOREPLACE (Linux RENAME_NOREPLACE/macOS RENAME_EXCL),
  never plain replacing rename or a check-then-replace fallback. It rehashes the
  actual retained source, validates single-link regular-file identity before and
  after the atomic effect, and acknowledges NewOwned only after exact file and
  directory durability/revalidation. An existing destination is ReusedExisting
  only after complete matching hash/size and stable no-follow file identity.
  Its receipt grants NO compensation/cleanup ownership. The unused owned stage
  stays separate and may move without replacement to unique recoverable trash;
  this is neither a purge nor published-file/import compensation.
- Atomic publication/native acknowledgement failure and post-rename flush or
  namespace substitution preserve files plus actual open-object witnesses.
  The retained stage becomes read-only and custody is sticky-fenced; dropping
  an error or guard never clears it. No blind republish, auto-Drop deletion or
  claim of no effects follows. Queued ordinary DB writes observe that fence;
  critical same-runtime writer-barrier readback, report/repair and drain remain
  available when the original root/identity/lease and DB-directory witnesses still validate, even if an archive-directory name is substituted. A complete
  guarded reconciliation/successor activation API remains a later concrete
  owner2/5/3/7 handoff, not claimed by these fresh-root mechanisms.
- Native profiles use their separately locked/serialized anchored namespace,
  not the library custody guard. First config publication is NOREPLACE; later
  replacement uses atomic EXCHANGE, preserving the displaced config in the
  private temporary name. Exact prior inode AND raw bytes are verified before
  owned-temp removal. A raced foreign config is retained rather than destroyed.
  Native marker/settings reads are anchored/no-follow, and parent substitution
  cannot redirect stage creation, replacement or cleanup. Work and cleanup
  failures are separately retained. Post-effect uncertainty or unsafe cleanup
  retains actual File witnesses in the poisoned store until restart; restart
  does not scan/purge orphan names or fabricate cleanup authority.
- File-data flush, parent-directory flush and atomic publication are separate
  mechanisms. On macOS both file and directory fsync are followed by actual
  F_FULLFSYNC via verified rustix safe wrappers; unsupported native guarantees
  return errors, never a weaker fallback reported as durability. This host's
  real operations succeeded. No authored unsafe or lint suppression is used.
  Hardware power-loss behavior, hostile same-UID sandboxing, native Intel/Linux/
  Windows execution and unsupported filesystem/provider behavior are NOT
  qualified by these isolated ARM tests. Existing library activation still
  requires4.7 independent-adoption authority, which is unfinished.

Combined ARM working-tree `cargo xtask check` passed107 nextest/zero skipped,
run69e0d40e-ddf1-44e6-8136-47fb2b779e2f, four compile-fail docs (three retained
plus the new retained-file/lease example), unchanged146-file preservation,
strict6 Python checks including admitted special-path wrapper regression,
root/fuzz deny+audit/inventory, locked build, formatting and all-target Clippy.
This final combined run includes fresh-profile identity retention, original
managed-directory recognition and critical DB readback during an archive-name
substitution. The preceding focused storage runff2cd654-1a80-4157-9709-dc1e39961092
also passed83/zero skipped. Neither run replaces current-head provider checks.
The15 new actual Unix definitions cover12 platform cases and3 profile cases:
destination/source/parent races; exact reuse versus aliases/changed bytes;
pre/post-effect cancellation; new/owned config races; cleanup substitution;
post-publication/trash flush uncertainty; retained actual witnesses; queued
ordinary-write fencing plus critical readback/close; poisoned profile restart.
Owner5 owns additive discovery scopes and any combined support checks. All
canonical4 checkboxes and full4.11/native gates remain pending admission.

### Windows exact-SQL failure investigation (4.2/4.5 follow-up)

Original provider Windows d6a966e run39478703189/job118821563565 is retained
FAIL:72 executed,67 passed,5 failed. The embedded-SQL byte comparison at
tests/migrations.rs:25 reports CRLF versus the admitted LF corpus; the prefix
inspection returns ForeignSchema. The three other observed RED assertions are
pending-prefix open (expected DirectoryDurabilityUnavailable), foreign-key
corruption (expected IntegrityFailed), and future/gapped ledger (expected
MigrationConflict). Their original assertions did not print the actual error,
so those three causes are not claimed independently established by that log.
They now retain the exact expected variants and print the actual refusal when
they fail. No SQL, schema comparator, expected outcome or target skip changed.

At consumed a2a2f7791d24c9e479bcb0b9a998ab9f6ab18a43, both storage/migrations
and internal/database/migrations SQL have unspecified text/eol attributes;
preservation/** is already byte-preserving. Owner5 alone owns the narrowly
scoped attribute/fresh-checkout repair. Each corrected Windows test result
remains required; ARM observations cannot qualify the Windows refusal branch.

### Partial 4.4: private frozen normalization translation

Consumed ORIGINAL owner5 support d6a966e→4910704→c6bc745→a2a2f7791d24c9e479bcb0b9a998ab9f6ab18a43
by fast-forward, retaining its source/notices checker, origin repinning and
additive discovery. Diagnostic follow-up ade1a38b9d4587d7d3f9b2b5ef84177c9b4d1621
keeps every Windows expectation unchanged and exposes actual refusal errors.

Private migrations/compatibility/** translates the specifically approved
Go1.26.5/xtext0.43.0 Unicode15 source/data: fixed UTF8 trie and decomposition
tables, original-character stream-safe partition BEFORE CCC ordering, Hangul,
the frozen1616 full-fold map, Mn-only removal and Go whitespace behavior. No
modern Unicode crate/composed stream_safe approximation or fixture alias is
used. The module is currently test-only; schema-only DatabaseOpen is unchanged.
CLDR32 Default parsing, strict v1–v5 query migration and the SEPARATE explicit
same-writer Collection Inspect/Open projection are still unfinished. This is
not a claim that the24 SQL-only payload/version/revision differences are closed.

The private source-manifest version1 records all9 actual derivative/glue/recipe
files, precise original identities/header hashes, changed representations and
required notices. Owner5's source policy passes. Product licensing is unchanged;
no language/DNS/TLD data is included in this step. Authenticated index SHA256 is
10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99;
xtext ZIP b75131f628f6e2e4892fd638d33127e952a5027775e0162b94ba069cb4e13863.

`verify_normalization.py` runs its authored qualification-only Go template in a
NEW independent git archive of exact73a93303/treee797e9d, with authenticated
487-member ZIP materialization, external modfile, pinned offline Go1.26.5 and
private caches. Before/after hashes retain all436 original archive files and
all487 module members; the frozen oracle, cache ZIP and selected Go sources
are unchanged. It never changes preservation inputs or supplies runtime Go.
Generated normalization-reference.json SHA256
3d1dfc4461b491a275168dfa07f77842d03df29e74384492b4841344c571c37e
retains actual tool/template/source identities and framed reference digests.

All1,112,064 valid scalars match actual Go for fold/NFKD/final Normalize/CCC/Mn/
Space. All147,228 sequences match input/NFKD/final Normalize digests, including
57 CCC/Mn representatives around12 boundaries and all18,019 decomposing or
backward-interacting scalars on both sides of29/30/31 nonstarters. The published
surviving-Mc counterexample (report7c41d90d…e375) has an explicit final-key/raw-NFKD
regression. These are scoped finite tests plus a source translation, not a proof
for every arbitrary string, language parser, Collection result or native host.

ARM focused final run1ae76f1c-57f8-41c4-99f3-3df99cfc45d7 passes all3 new
normalization tests. Prior cumulative `cargo nextest run --workspace --all-targets
--locked` run880ce876-948f-4408-a54e-9a42378709ee passed113/zero skipped (includes
the3 benchmark checks and preceding39,114-sequence reference); final focused
run expanded that reference to147,228. Four separate compile-fail docs, exact146
preservation corpus, private-source policy, storage all-target Clippy and fmt
pass. Owner5 retains additive test discovery, root files and any combined check
support. Windows corrected-head execution and full4.11 remain open.

### SQL checkout repair consumed; original Windows failures resolved in scope

Private normalization source commit a125778200af8d28e8ff99c6c41f47a6ef5b69db
passed its EXACT committed ARM `cargo xtask check`:110 nextest/zero skipped,
four compile-fail docs,146 corpus, strict14 Python, both graph policies,
source/notices, formatting/build/Clippy; run16161520-6594-48da-8b0a-8b56d24b3383.

Consumed/published ORIGINAL5686c82a912da4ebf7e02f866de5311cff40158b by FF.
This retains original16973ca2003b7d0a64135f3f25e3b137754aac85 and merge88a7eb4
with a125; only6 owner5 root/tooling files differ from a125. No source SQL,
comparison, expected outcome or byte pin changed. Independently read full6184B
owner5 migration-byte support report SHA256
71ce4d35034a4d370a24e627a9b2a14ac76c4a3c21af63b8b878d471b1c49ad5,
stable before/after. It supplies all5 pre-fix ARM autocrlf reproductions and
corrected fresh-checkout7 backup/5 migration passes,54 exact SQL triples and
unchanged controlled-parent/Git bytes. This closes the previously unproven
scoped conversion attribution with supplied reproductions, never normalization.

Independently inspected CURRENT5686 completed native Windows job118840366387,
run39483994204:75 passed,zero skipped. EACH original RED is individually PASS:
compatible prefix17/75, foreign-key corruption18, future/gapped21, embedded
bytes46, all-prefix recognition55. The fresh persistent-autocrlf special-path
embedded assertion also passes19:32:35Z. Provider checkout1edd3d1b6fdffb10b01707539321b81387d5eb6f
has verified parents438+5686 and tree d9ace6bd5ff9da2a0f6a43661cf7f9bad9472781,
identical to published5686. Original d6 failure remains historical evidence.
This is actual Windows retained/refusal-test execution; the pending-prefix
DirectoryDurabilityUnavailable refusal is not an implemented4.9 capability.
Intel/current full gates and future source heads are independently qualified.

### Partial 4.4: private frozen Default-language translation without DNS

The private test-only compatibility module now translates the bounded saved
query language boundary, Go Default=Deprecated|Legacy grammar/aliases, known
language/script/region/variant data, lazy scanner behavior, grandfathered tags,
extensions and exact compact String round trip. It preserves original-to-
derived mappings and existing unassigned product-license status. It never uses
modern aliases, SuppressScript/Macro/CLDR defaults, parent/matcher heuristics,
runtime Go, TLD APIs or the unapproved ccTLD flag. The literal region flags are
masked ONLY at bit1/value2 before emission; all other bits are retained.

Actual offline pinned Go1.26.5 execution in NEW independent exact73/archive+
authenticated487-member inputs was compared with a SEPARATE owned module copy:
mask359 region flags and remove BOTH public/internal Region.TLD methods. Only
those3 selected members differ;484 remain byte-identical. Raw Default Parse
Tag.String, exact Und equality, full returned error text AND required product
canonical outcomes are identical on1,369,529 inputs. The original436 archive
files,487 module members, source ZIP, selected Go files and tool remain unchanged.
language-reference.json SHA256
a687f58350d126addd110fb8b6aa6f88489ccfb05e4dc6cd401de92a99ce45b8
records source/tool/probe/seed identities, precise counterfactual hashes and
framed reference digests. Original/masked Go outputs are compared in full.

Rust matches actual Go's product canonical/refusal outcomes and input digests:
346,788 base/suffix cases (all2/3-letter bases),456,976 scripts,19,252 regions,
523,900 compact-region overrides,12,544 variant pairs,10,000 deterministic
grammar sequences and69 explicit edge cases. These include alias distinctions,
duplicate keys/variants, transform truncation, private use, Und, compact bit
widths and region override elision. The product rejects every parse error;
private Rust exposes no partial error tag as a new public API. Source inspection
and finite differential tests support this translation, not every possible
string or full Collection behavior. No schema-open rewriting is activated.

Focused ARM run8db2c061-6cbd-4871-a96a-a3fb11e384af passes all7 private
normalization/language tests. Replaying the shared independent-input helper
keeps normalization-reference.json byte-IDENTICAL at3d1dfc44…c37e, retaining
all1,112,064 scalar and147,228 sequence checks. The version1 private manifest
inventories16 exact files and their original headers/notices; source policy,
storage all-target Clippy, fmt and unchanged146 corpus pass. Final committed
cumulative/native checks follow separately. Strict saved-query v1–v5 decoding,
atomic explicit same-writer Collection Inspect/Open and all24 projection
assertions remain REQUIRED and unfinished;4.4/4.11 are not complete.

### Partial 4.4: strict private v1–v5 saved-query wire codec

The still test-only inline `migrations.rs::collection_codec` separates invalid
payloads, unsupported versions and frozen-data invariant failures. Its bounded
ordered wire parser retains duplicate fields and numeric lexemes, Go bytewise
invalid-UTF8 replacement, unpaired-surrogate replacement, null assignments,
unknown-field/trailing-document rejection and recursive limits. Frozen Go15
simple lowercase (duplicate keys) remains DISTINCT from ASCII-reaching Unicode
simple-fold orbits (struct fields); full search folding is never a wire-key rule.
Validation precedes legacy Broken removal; v4 retains ordinary reused IDs, v5
rejects built-in flag restrictions. Canonical payload field order, Go HTML/string
escaping, raw array/group limits, exact CLDR32 language outcomes and the source's
repeated recursive normalization passes are retained. No evaluator, Collection
CRUD, shared API, eager DatabaseOpen or production projection is activated.

Qualification runs actual Go1.26.5 DecodeSavedLibraryQuery AND
MigrateSavedLibraryQuery, compiling a byte-IDENTICAL temporary standalone copy
of frozen collections/query.go SHA256
569189457f91d9f29980a51fa7d68040745b07d7bddbb2d71dfc03c8dc89b763.
Original library/collectionquery/searchtext algorithms remain unchanged. On
2,864 deterministic scoped cases Rust matches separate decoded/migrated bytes
and invalid/unsupported outcomes;723 are accepted in each phase. Inputs cover
all six current/unsupported versions, retirement authority0/7, null/scalar/array
shapes, Unicode key aliases vs duplicates, integer boundaries/-0/exponents,
invalid UTF8/surrogates, branch/filter/value/name ceilings, actual surviving-Mc
counterexample and400 structured adversaries. The recorded recipe verifies all
436 independent exact73 archive files and487 authenticated xtext ZIP members
unchanged; no frozen oracle execution, downloaded module, production data or
preservation generator/corpus writes. `codec-reference.json` SHA256
b2fd01d9685133f295f1fbe23296d50971f07314ed94ed18d6a235903bca0d09.

The actual Go scalar reference now additionally qualifies simple lowercase and
ASCII-reaching simple-fold rules over all1,112,064 valid scalars; all six prior
scalar metrics and147,228 sequence digests remain byte-identical. New reference
SHA25661a5a904eae2e88d07aa667f5cd2e0d74dd83a7c2e247e87d46b621f427a2468.
The explicit derivative manifest inventories19 files with original headers,
recipes/hashes/notices; source policy, storage all-target Clippy and fmt pass.
All95 storage nextest tests/zero skipped pass ARM, run
13e2d196-dd8d-4be2-ba2f-f7b273390c25. Cumulative checks follow separately; five
new actual all-target identities require additive owner5 discovery support.
Finite differential evidence is not arbitrary-input, SQL projection or native
filesystem qualification. Separate same-writer Inspect/Open, all18 phased
prefix snapshots and all24 SQL-only payload/version/revision differences remain
unfinished and REQUIRED for4.4; all4.11 admission gates remain open.

Current9dd automatic workflows are scheduled/pending. Superseded5686 Rust
quality workflow was CANCELLED after9dd; its individually completed Windows75
and corrected five assertions remain exact-head historical evidence, never
all-eight or current-head qualification.

The combined WIP workspace check passes119 nextest/zero skipped, four separate
compile-fail docs and unchanged146 corpus. This is ARM execution; exact committed
checks and serialized discovery follow, not a four-target qualification.

### 4.4: explicit same-writer Collection compatibility projection

Consumed ORIGINAL6be041206d7f2af09f09c02fc75fe0639cef486d by FF, preserving
all116 prior discovery identities/scopes/metadata and the five actual strict
codec identities. Owner5 supplied exact-head ARM119/zero skipped+four docs,
146 corpus/strict15/source/both graph policies. Windows84 is REQUIRED inventory,
not local execution. No root policy, dependency, source notice or SQL byte edit
was authored by this session.

The separate `Database::inspect_collection_compatibility` uses THIS runtime's
existing writer and transaction/cancellation/commit-uncertainty mechanisms.
DatabaseOpen still performs schema-only upgrade with a verified backup, never
lazy domain projection. An explicit inspection reads actual Collection+retired
Broken identity, migrates/decodes with the single private codec, validates real
recursive tag kinds and choice-value owners, then atomically updates exact
payload/version OR attention. Unmigratable/missing-criteria domain outcomes are
distinct from SQL/invariant failure. Existing needs_attention is never healed.
Unchanged/repeat inspection is a no-op. The unchanged migration016 trigger alone
increments PERSISTED revision; returned int64 bookkeeping is not another SQL
update. Raw legacy text bytes stay beside the owned storage-local record. These
transitional results are not new shared app DTOs or a frozen section7 API.

On ALL18 independent owned copied prefix databases, actual DatabaseOpen+backup/
upgrade schema, triggers, constraints and every non-Collection row match the
admitted Go oracle. Schema-only preserves ORIGINAL query bytes/version and
retains EXACTLY24 observed payload/version/revision differences across11–18.
The SEPARATE explicit same-writer inspection closes ALL24; full snapshots and
openedCollections domain/query outcomes equal the immutable JSON. No fixture
payload branches, input changes, eager DatabaseOpen semantics or deferred
Collection implementation were used. Test-only timestamp fixing follows EXACT
existing generator/databases.go::fixTimes (the declared producer policy) AFTER
checking real CURRENT_TIMESTAMP effects against current SQL clock bounds.
Repeated inspection preserves the full first-result snapshot including time.
Production always uses CURRENT_TIMESTAMP; payload/revision/domain comparison
bytes are never normalized.

All-target effect tests cover valid/canonical/repeated inspection; existing
attention; missing/wrong-kind/foreign-owner recursive criteria; actual retired
vs reused IDs; missing legacy-reference SQL; unsupported/invalid codecs; not
found; pre-admission and late cancellation; AFTER UPDATE payload/revision AND
attention/revision aborts. An actual deferred-FK COMMIT failure is
CommitUncertain, fences ordinary retry, retains same-generation runtime/lease,
and permits ONE fresh same-writer readback plus reserved uncancelled repair.
Observed rollback comes from that authoritative view, never the error alone;
readback/repair does not silently clear the fence or reactivate intake.

ARM focused6 tests/zero skipped PASS, run
ecf614a5-3a6f-47e3-ada1-ea9d238e9daf. Combined workspace125/zero skipped+four
compile-fail docs+146 immutable corpus PASS before the additional attention-
abort assertion; final focused6 includes that assertion. All-target storage
Clippy/fmt and explicit source19-record policy pass. Source extraction replay
from the authenticated49 materialization reproduces the new cfg(test)-only
region-flag witness and retained non-DNS bits; no algorithm/data substitution.
Five new effect identities are all-target; the actual all-prefix backup/open
proof is Unix-only while Windows durability4.9 is unimplemented. No compile or
refusal branch is native filesystem qualification. Exact committed full
cumulative/new-head provider gates and independent4.11 review remain pending;
no full4.4 admission or PR43 review/merge authority follows from these passes.

Exact committed e9 passed `cargo xtask check` on AppleARM1.99.1:125 nextest,
zero skipped, four separate compile-fail docs, unchanged146-file corpus,
strict15 Python and source19-record/both graph quality policies. Nextest run
`c2c3d73c-2b7e-453c-bb23-3e92489a8cb0`. Original owner5 discovery e9 retains
all121 prior identities/scopes/metadata and adds only the six actual projection
identities. This closes the preceding host-check distinction; Windows89 remains
required inventory until actual execution at the relevant head.

Integration owner2 independently checked a clean exacte9 checkout with a
temporary actual-API harness: all19 admitted DBs, full18 prefix snapshots and
openedCollections, the24 separate schema-only vs explicit-open field effects,
representative state/raw history, eight SQL-clock bounds and nine repeat/no-op
checks. Separate source-composed cases exercise cancel/abort rollback, actual
COMMIT uncertainty, fenced retry, fresh readback and reserved repair. All771
tracked source hashes remained unchanged. The7582-byte report SHA256 is
`254e6d3460b95e2dd5fd64c6f9acc6e38d18791a5d94ffa67b60a1d8ba06ed0c`;
owner6 and root read/hash-verified the complete report. This finite implemented
slice is not full4, final4.11 independent native review or merge admission.
There is NO semantic nonmutating `InspectInTransaction` counterpart in storage:
the explicit API mutates lazily; a generic fresh barrier read is only a mechanism.
That distinct semantic seam remains explicit for a future owner5/domain handoff.

### Partial 4.7: closed independent input verification, no activation authority

`ClosedManifest` retains exact source bytes and rejects ambiguous relative paths,
duplicate/unlisted or incomplete entries, invalid size/hash data and live SQLite
sidecars. `ClosedLibraryInput` is a read-only retained Unix input witness, not an
IsolatedLibrary, OS lease, source cleanup permission or profile commitment.
Verification scans the complete tree through retained no-follow directory/file
handles, checks lossless names, regular/nlink1 files and bounded streaming hashes,
and probes known schema/integrity/FKs only on a NEW private database copy. The
source database is never opened by SQLite, including read-only WAL/SHM access.
The raw marker, profile bytes/UUID spelling, manifest, state/history and every
archive remain untouched. A second complete scan and fresh revalidation detect
change; these observations do not claim continuous exclusion of old Go.

Directory enumeration uses independent safe-rustix cursors; file descriptors do
not accumulate per manifest entry. Cancellation is checked before source I/O and
during enumeration/hash/copy. Unlisted WAL, modified/missing entries, root
replacement, aliases, profile spelling mismatch and self-consistently hashed
FOREIGN SQLite all refuse without repair or cleanup. A compile-fail check proves
the input witness cannot open a writable runtime. In-place legacy adoption still
refuses before any I/O. No journal is replayed or rebound by input verification.

Seven focused tests/zero skipped passed AppleARM, run
`39bb9497-9b6f-442c-9c01-6bb1e4a9da9c`; all three storage compile-fail docs and
all-target storage Clippy/fmt passed. The two manifest tests are all-target;
the five descriptor/probe tests and new compile-fail are macOS/Linux only.
Combined workspace132 nextest/zero skipped (run
`cc60a3b7-577c-423f-a7d4-318692f69173`) and five separate workspace compile-fail
docs pass on that host; additive discovery is serialized through owner5.
Independent copy/backup, separately owned native namespace/profile/recovery,
no-alias activation, crash/restart/relocation and later legacy-launch evidence
remain REQUIRED and unfinished. No input tuple or successful probe grants those
capabilities. Final cumulative/native4.7/4.11 gates stay open.

### Partial 4.7: independently owned inactive copy

`ClosedLibraryInput::copy_to_new_root` exclusively creates a NEW anchored target
outside the source tree. Its own OS lease is acquired before library bytes are
copied; the original directory handle creates that lock without writing through
a substituted root name. All regular files use descriptor-relative CREATE/EXCL,
bounded streaming and copy rehash, nlink1/identity checks and file durability.
Complete source and copied tree/name/hash scans plus another private SQLite
probe verify the declared snapshot. Child directories flush before their parent,
then the root name flushes in its retained parent. No source identity, settings,
DB, state, history, archive, recovery file or manifest is rewritten. Directories
are0700 and new files0600, with distinct device/inode identities from the source.

The returned `IndependentCopy` is INACTIVE, with no conversion to IsolatedLibrary
or Database and no native profile commitment. Copied absolute recovery/history
paths remain raw evidence, never cleanup authority. `IndependentCopyFailure`
retains actual directory witnesses and the acquired lease alongside its error;
no destructive Drop removes partial/uncertain bytes. Its last observed root path
is explicitly untrusted after substitution, not a cleanup receipt. Abandoning an
inactive copy releases handles while leaving all bytes; it cannot activate it.

Four additional Unix identities verify exact copied bytes/independent inodes,
source-only later activity, cooperative second-lock refusal/release, collision,
source overlap/change, pre/late cancellation, a private final-flush fault seam and
root substitution preserving BOTH original copied and unowned replacement bytes.
These are synchronous real filesystem effects with a fault injected immediately
before final directory acknowledgement, not physical power-loss qualification.
All136 workspace nextest tests pass/zero skipped on AppleARM1.99.1, run
`99ec6fd3-0364-4061-b6e9-dc8b2a691b19`; five compile-fail docs and storage
all-target Clippy/fmt pass. Additive owner5 discovery and current provider/native
checks remain separate. Safe native profile/recovery rebinding and activation,
crash/restart/relocation and unmodified legacy-launch evidence are still REQUIRED
before independent adoption can be declared complete. Section17 stays separate.

### Partial 4.10: foreign profile inspection and rebinding data

`ForeignProfile` separately inspects explicit POSIX vs Windows drive/UNC root
syntax in the strict64KiB version1 wire format. It retains original bytes and
raw UUID/root spelling without path resolution, profile discovery, defaults,
library opening or journal effects. Extended/device namespaces are refused for
rebinding; ordinary absolute syntax alone is not native Windows path or FS
qualification. Existing native-host `ProfileDocument` validation is unchanged.

`prepare_rebinding` borrows an actual verified IndependentCopy/lease, revalidates
its complete tree and EXACT raw UUID spelling, and prepares a distinct native-host
profile DOCUMENT with the copied root. Language/version/complete/recovering and
raw UUID are preserved; source bytes and source authority remain unchanged.
An unrepresentable native byte root refuses instead of lossy conversion/renaming.
There is no publish/runtime/cleanup method: actual native namespace/activation
and copied journal rebinding/refusal remain separate requirements. No operation
history/domain SQL is inspected or interpreted as cleanup authority.

Two focused tests pass/zero skipped on AppleARM, run
`4ff5e36e-a079-4436-a902-3ae755eb91bf`, and storage all-target Clippy/fmt pass.
The all-target pure test covers explicit syntax, unknown/version/language/
trailing/size refusal with original bytes retained. The Unix copy test verifies
raw-ID case mismatch, cancellation, unchanged source state/history/profile,
no created settings/recovery/journal and refusal after an unlisted candidate
file appears; it preserves that unowned file. This does NOT qualify filenames,
case/Unicode collisions, native Windows I/O, copied relocation cleanup or full4.10.
Additive discovery/current combined/native/final4.11 gates remain open.

Inactive-copy implementation is retained in
`c4a3d0d914db88b4a2e18302f4eb907d51ca47c1`; the partial foreign-profile document
work is `d19b113b497d2e18a5477dc4ca606adcbfb7858a`. Copy failure now separately
retains the attempted target NAME even if mkdir succeeded but obtaining a child
handle failed. Absence of a retained child/lease is never claimed as no-effect
proof; observed names confer no cleanup/activation/retry authority. Existing
collision, overlap, precancel and retained-lease failure tests assert those
distinct facts (focused11 pass, run`cbcf591c-7046-403b-a0ea-ef5bd72ab164`,103
tests intentionally filtered). Storage all-target Clippy/fmt pass unchanged.

### 4.3 correction: failed queue stop must still join actual owners

Consumed ORIGINAL owner5 support`37aaff34af486b710cd5e2e3918893b11e9450ab`
by FF from0a, retaining01bc/4d71668/6426c2f/406829fe. Net change was ONLY
baseline+102/-0: all127 prior entries/metadata/scopes preserved plus13 actual
input/copy/portability identities (140 static entries). Own exacte37a ARM
`cargo xtask check` passed138/zero skipped+five compile-fail docs/146 corpus,
strict15/source19/both graph policies, run`5c589c1d-e537-4071-8015-04454dda60eb`.
No manual root edit, recreated support, cherry-pick or bookkeeping push.

A separate real isolated regression reproduced early return from `close` after
admission-mutex poison: the original fallible queue-stop `?` left unjoined read
and writer handles. RED run`0942f6ee-3e55-4d51-bbe6-c55a30b0e4ed` failed at
"close returned before joining readers". This is NOT an old clean-close proof
or a synthetic witness for the missing threaded jobs/resource bridge.

Close now accumulates queue-stop errors, stops/wakes both lanes even with poison,
and joins EVERY actual owner before returning fenced failure. Poison recovery
only permits stop/wake and discarding queued callbacks outside the mutex; it
does not execute suspect queued SQL, clear a fence or acknowledge their effects.
Discarded senders yield unavailable results. No public API or domain SQL changed.
The same regression exercises BOTH poisoned reader and writer lanes, verifies
no retained join handles, explicit failed/not-closed state, refused runtime
reopening and OS lease held beyond failed DB close until the library drops.
It never labels close failure as successful durable settlement.

Initial focused GREEN run`b0574597-c6e2-4b03-b1fa-9d921787778d` passed the reader
case; final combined BOTH-lane check passes139 nextest/zero skipped, five separate
compile-fail docs and unchanged146 corpus/strict15/source19/both graph/fmt/build/
Clippy/discovery on AppleARM1.99.1, run`074890f8-0e66-40fe-b44c-0019dfc6153b`.
One new all-target actual identity requires additive owner5 pinning. Other native
checks and any dependent isolated failure/drain proof must consume this corrected
head; no old result carries forward. The static owned jobs bridge, NativeLibrary
activation, domain repositories and final4.11 gates remain separate and open.

### Partial 4.7: owned native profile binding, no SQLite activation yet

The preceding poison-close correction is `a6fd4677e3069cef0d59300da77ba17dc1ae6215`.
Consumed ORIGINAL `44185b177603e1064eb05b05b41c18ac704e7bdc` by FF: only its
one actual all-target poisoned-close identity was added; all prior entries and
scopes survived. Published manifest-only `e731244d1a161337335f1a878929ad7ef73f7634`
then consumed ORIGINAL owner5 direct-descendant
`d710fd226c8c0cbd41a192f89d7f2917dccab202` by FF: ONLY root Cargo/lock+one line
each, exact windows-sys0.61.2/defaultfalse/six approved features including
Win32_Security; no version/source replacement. The intermediate e731 Linux
job118898100415/run39500781805 failed BEFORE tests because workspace.windows-sys
was absent. That concrete graph failure remains historical; no unchanged retry,
waiver or prior-head native pass substitutes for the corrected combined graph.
No Windows wrapper/unsafe, directory-flush contract or config mutation is
implemented or qualified by this dependency synchronization.

`native.rs` now consumes the ACTUAL IndependentCopy and retains its original
root/parent handles and OS lease, together with a borrow of the marked native
profile namespace/OS-lease owner. It requires disjoint explicit native config,
native data and source/legacy contexts, and the copied root inside that native
data namespace. Raw marker, UUID spelling and source profile/library bytes stay
unchanged. A fresh native profile inherits the source language; a newer native
language choice is retained. Matching current native settings are not rewritten.
No ambient discovery, TestRoots cast, in-place legacy opening or profile-data
boolean conveys this capability.

The private binding's Arc identity excludes concurrent generations. Active root,
raw ID, complete and recovering fields cannot change through update(&self);
language can update under the short existing settings mutex. Complete copy
verification/copy/SQL never runs under that mutex. Real post-replace uncertainty
poisons the cache AND binding; changed copy/profile/root names or newly introduced
unverified journals fence it. Dropping the binding cannot clear this fenced
exclusion. All errors retain the actual copy and profile lifetime; no destructive
Drop, fake rollback or path-based retry follows. Known copied journals/move-owner
markers, recovering source/native data and extra native recovery artifacts are
refused without reading/replaying/cleaning their foreign destinations.

This finite phase does NOT open SQLite. Database::open_native, same-worker
native runtime admission/repair/close integration, retained unresolved runtime
exclusion after failed close, restart/relocation/recovery rebinding and the
owned threaded jobs bridge remain unfinished. The new compile-fail assertion
prevents passing NativeLibrary to open_isolated; it is not proof of those
missing runtime mechanisms. Windows native binding is unavailable until the
actual platform capabilities qualify. Domain3/7 namespaces stay inactive.

Five actual Unix integration tests and one Unix profile-only fault test cover
both real leases, raw bytes/no WAL side effects, root/ID/recovery exclusion with
concurrent language updates, two competing bindings, retained losing copy,
cancellation, exact refusal of copied/unrebound journals, root substitution,
external profile changes and post-replace poison surviving guard drop. The
first full check exposed an actual race: an unlocked namespace scan could
mistake a concurrent binding's live settings temporary for foreign recovery.
The short scan now runs INSIDE the settings mutex after active-binding refusal;
no assertion was weakened. Focused corrected run
`9c675d47-1694-4de1-a00e-9780b79cb276` passed5/zero skips.
Final source-combined AppleARM1.99.1 cargo xtask check passed145/zero skipped,
six separate compile-fail docs, unchanged146 corpus/strict15/source19, both graph
policies/fmt/build/all-target Clippy/discovery:
`9aa34c86-30b6-4189-a7fa-35953059d78e`. Only this ledger was added afterward.
Owner5 additive pinning of the six actual Unix test identities follows from the
published source. Other native/current provider/exact-head review/full4.11
admission remain open; no physical durability or full section4 claim follows.

### Partial 4.7: explicit native SQLite ownership on the same workers

The finite native binding commit is
`e38f131919bddf065fdc2c3aa3dcb65e4f69421c`. Consumed ORIGINAL owner5
`c4cce59fd07cd35e418239b902d38bbdc33bab66` by FF, baseline-only six Unix
identities; all141 prior entries/order/scopes retained. It was not rebuilt or
cherry-picked, and the distinct terminal-SQL temporary proof stays pinned to
S=d710/a6 rather than silently consuming this later native branch.

`Database::open_native` now borrows NativeLibrary through actual worker closure.
A private two-variant authority enum dispatches to the SAME writer/four-reader
code and queues; no public authority trait, second pool, lifetime cast, leaked
owner or static/Arc jobs bridge was introduced. Native initialization checks the
original root/parent/OS lease, exact marker bytes/raw ID and retained original DB
directory, then opens/creates only the missing managed directories relative to
the retained root FD. Their actual objects survive through close. File/directory
flush errors or partial initialization retain the fenced owner; no recursive
cleanup or blind reopen follows. No source database/profile is mutated.

Native profile publication/identity uncertainty and uncertain COMMIT share the
actual generation fence, checked again before queued ordinary writes execute.
Required repair/barrier/backup uses the same bounded four reserved FIFO slots,
retained root/marker/DB-directory ownership and ONE fresh writer transaction;
it remains possible while settings/archive ordinary work is fenced. A successful
DB close drains/joins/checkpoints actual connections. It does NOT clear native
uncertainty. Failed close retains the generation fence and runtime claim, and
NativeLibrary Drop cannot convert either into a settled replacement generation.
Safe borrowing keeps root/profile leases alive beyond every DB join; ordinary
work cancellation cannot discard those owners or cancel required repair.

This integration also closes a concrete e38 authority gap: public ProfileLocations
fields are data. They cannot relabel a fresh Go-discoverable settings directory
as native. Both explicit and canonical native config/data basenames must match
the accepted host namespace, with disjoint source/legacy locations. A source
relocation journal is refused read-only, never followed or repaired by rebinding.
The regression uses a fresh synthetic Go-named profile directory and verifies
refusal; positive fixtures use actual for_host locations under synthetic roots.

Five new Unix runtime integration identities exercise actual five-worker
WAL/FK/5000 diagnostics, distinct native/development admission, clean close/reopen,
source/marker/archive preservation, real deferred-FK COMMIT uncertainty and fresh
writer readback, uncancelled repair, and persistent exclusion across close/drop.
A real writer panic loses its acknowledgement and fails close honestly; all
actual joins run before lease release, unlike a fabricated closed result. A
retained-original DB-directory substitution refuses before SQLite touches the
foreign bytes. A bounded test-only queue gate then proves that a profile change
preserves an already committed fact, refuses the queued callback before SQL,
and still permits same-generation readback/repair. Existing six binding tests
remain cumulative; no count or assertion was removed.

Focused10 native tests passed (zero skips), run
`36603287-d46d-4617-9820-7b52f328d885`. Full source-combined AppleARM1.99.1
cargo xtask check passed150 nextest/zero skips + six separate compile-fail docs,
unchanged146 corpus/strict15/source19, both graph policies/fmt/build/Clippy/
discovery, run`de0e3fe2-f88e-4439-8381-4e340ca7270a`. Only this ledger was added
afterward. Owner5 will pin the five actual new Unix identities and updated doc
locations from the published source; prior baselines remain intact.

This is not full4.7: restart authority/recovery replay and relocation remain
unfinished; a profile loaded after restart is still DATA, and no existing-root
native reopen constructor exists. Native archive/artwork custody adapters,
static jobs/resource transfer, operations/import SQL and full4.10 preflight are
separate. No Windows wrapper or unsafe code was authored; directory durability
and config mutation remain unqualified/excluded. Current provider/native/exact-
head independent4.11 review and complete cumulative admission remain required.

### Partial 4.9: private Windows handle identity observations, native gates pending

The native same-worker implementation is
`4507da6afff4896e21a4a1f2e17725fdc724fb08`. This next slice uses ONLY the already
approved query_identity subset from the exact d710 graph. Independently hashed
windows-sys0.61.2 archive:2517186 bytes,
`ae137229bcbd6cdf0f7b80a31df61766145077ddf49416a728b02cb3921ff3fc`.
No root/lock/features or external source selection changed.

One PRIVATE safe function borrows a live std-owned File and synchronously queries
GetFileInformationByHandleEx with exact initialized tag/128-bit-ID/basic/standard
buffers. A function-local unsafe_code expect has a reason; EVERY unsafe call has
its own SAFETY ownership/alias/lifetime/alignment/size/completion explanation.
No unsafe fn, blanket lint suppression, raw-handle transfer or async pointer
exists. The standard-information BOOLEAN receive fields use initialized u8
repr(C) storage with actual-target size/alignment/offset assertions against the
pinned SDK binding, avoiding Rust bool invalid representations. API failure
captures last_os_error immediately; reparse/deletion/type/link observations are
refusals, not cleanup authority. Volume+128-bit IDs describe retained live
objects, never permanent identities after deletion or coherent content proofs.

Primary contracts inspected for implementation:
https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info
https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_standard_info
The initial fileapi URL returned404; the inspected authoritative function page
is winbase above. No research-spike experiment or candidate/pin was duplicated.

Fresh-root LibraryLease now retains additional read-only Windows root/lock-file
metadata handles and compares their actual IDs with newly observed names and the
real locked handle. Safe std open flags avoid following the terminal reparse
object and deny delete-sharing for those pins; they do not grant handle-relative
ancestor traversal, publication, mutation or directory flush. Older timestamp
checks remain cumulative. Legacy lifetime exclusion is STILL unproven/refused.
The NativeLibrary route is STILL unavailable on Windows.

Four source-defined Windows-only tests require actual same-object/different-name
IDs, content changes with retained identity, deletion/rename refusal through
actual handle release, hardlinks, reparse refusal, wrong-kind/missing objects
and bounded concurrent queries. The reparse test MUST fail if its synthetic link
cannot be created; it never skips or converts that into a passing qualification.
They have NOT been compiled or executed on this AppleARM host. Actual native
Windows ABI/behavior/failure/race execution and independent review remain
mandatory; a future corrected head needs its own provider evidence.

Host cumulative cargo xtask check passed150/zero skips+six compile-fail docs,
unchanged146 corpus/strict15/source19/both graph policies/fmt/build/Clippy. That
run excludes cfg(windows) code and supplies NO Windows compilation/execution
claim; subsequent changes only tighten Windows-only metadata checks/test
assertions and append this ledger. Directory durability, NtCreateFile child
ownership/no-replace move, config replacement and full4.9 remain OPEN. No
NtFlush substitute, unsupported WRITE_THROUGH claim, source/fixture change,
physical Windows qualification, early4.11 review or merge is implied.

### 4.9 correction: attribute-only access does not pin Windows names

Actual published65ff native Windows job118909880578/run39504409923 compiled
the four handle-query calls and ran97 nextest cases:96 passed, one failed,
zero skipped. The retained-handle rename assertion at windows.rs246 failed:
rename succeeded while the FILE_READ_ATTRIBUTES-only witness remained open.
The other three new Windows identities passed, including reparse/hardlink and
concurrent-query cases. This finite result does not qualify the failed pin or
complete4.9. Original job log is retained at
`/private/tmp/librairii-65ff-windows-native.log`, SHA256
`2c1586047e36dde51197f3a20f94e1f15f06df965c329180244121f550b7c3b8`.

The CreateFile contract exempts attribute-only access from sharing checks:
https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
The safe std witness now requests READ_ATTRIBUTES plus READ_DATA for a file or
LIST_DIRECTORY for a directory, retaining READ|WRITE sharing without DELETE.
Denied data/list access refuses; there is no attribute-only fallback. No new
FFI, graph edge, parent traversal, mutation or directory-flush claim is added.

The existing native test keeps every rename/delete exclusion assertion. It now
also contrasts an attribute-only handle that permits rename, reads exact bytes
through the actual file pin, and checks directory rename/delete refusal until
the actual list-directory handle is released. Its existing discovery identity
is retained; no old assertion, test or target scope was removed. Original
121be discovery-support ancestry is preserved. Corrected-head native Windows
execution remains required; this AppleARM host cannot supply that evidence.

Source-combined AppleARM cargo xtask check passed150 nextest/zero skips and six
compile-fail docs, unchanged146 corpus/strict15/source19 and both graph/quality
policies, run`85f9889b-746e-4f2a-8d64-fcb9c85a324c`; only this validation note
was appended afterward. This host check excludes Windows-only source. It is
not a green native rerun of the original failure or proof of directory durability.

### Partial 4.10: streamed original-name diagnostics, no target admission

`portability::windows_name_issues` borrows the already bounded ClosedManifest
and streams original path/component observations without a collected issue
allocation or filesystem I/O. It reports documented reserved/control characters,
device stems including superscript COM/LPT digits and extension forms, and
trailing spaces/dots for both file and directory components. Raw strings and
manifest bytes are unchanged; no shortening, replacement, Unicode normalization,
casefold model, profile publication or journal interpretation occurs.
Primary rule source inspected:
https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file

The type has no adoption/cleanup/all-clear capability. Empty output proves only
absence of the documented lexical issues detected here. Case/Unicode equivalence,
path/component lengths and APIs, actual links/reparse points, network/removable/
offline/read-only filesystem guarantees and recovery authority remain separate
preflight obligations. Declared data cannot prove that a source object exists.
Other absolute profile/history/journal strings remain unchanged historical data.

Two all-target source-defined tests use synthetic in-memory additions to the
unchanged admitted manifest. They retain exact device/control/directory spelling,
compound issues, composed/decomposed Unicode and case pairs, embedded spaces,
percent/hash characters and non-device lookalikes without claiming that any pair
can coexist on a real target. Focused portability4/4 passed, zero skips, run
`a8ade1af-9c4d-4acb-8731-53bc2fdc2a1d`; storage all-target Clippy with warnings
denied passed on AppleARM. New discovery identities require owner5 additive
support before final admission; this is not full4.10 or current all-target proof.

### Partial 4.8/4.7: native filesystem owners retain the actual SQLite borrow

`LibraryFiles::open_native(&Database)` now requires the actual open-native DB
variant and its sealed NativeLibrary. A private two-variant authority reuses
the existing Unix descriptor-relative stage/hash/no-clobber/reuse/flush/temp-trash
mechanisms. Native roots are duplicated from the original owned root FD, never
reconstructed from a root/ID/profile DTO. No second pool, connection, public
authority trait, shared app port, domain SQL, unsafe or static jobs bridge exists.

Files AND the move-only Custody guard retain both actual NativeLibrary and SAME
Database borrows. A guard may outlive its original Files instance without losing
that DB borrow. Stages and all publication/reuse/uncertainty receipts borrow Files
and keep its real directory/DB owners alive. Mutable DB close/join therefore
cannot precede file-owner drain. Three new compile-fail cases cover an active
stage, a guard after Files drop, and attempted Files drop with a publication
result still alive. Successful synchronous integration cases separately compile
and exercise the corresponding real ownership flow.

All native instances share one custody bit on the real library. File uncertainty
uses the SAME native profile/DB effect fence; guard Drop never clears it. Native
DB/local worker failure refuses ordinary file effects. Closed/isolated DB modes
refuse this constructor; no cast/reopen fallback exists. Short queue-state reads
stop new Files/guard intake when ordinary DB intake stops. Existing guarded
resolution and bounded same-writer readback/repair remain possible after intake
stop; no mutex spans file/SQL work and no cancellation discards these owners.

Three new Unix integration identities cover publish/exact reuse, a guard moved
beyond Files, second-claim refusal, prepublish cancellation with retained bytes,
uncancelled retirement of owned temporary bytes after intake stop, unchanged
reused bytes, and real DB close only after file drain. Actual deferred-FK COMMIT
uncertainty fences file writes but preserves readable retained bytes, fresh writer
readback and repair; guard/Files/DB drop cannot activate a replacement generation.
An original archive-directory substitution refuses before staging, preserves
foreign bytes and retains DB-only readback. Source SQLite/archive bytes and
native root/profile leases remain retained independently of ordinary work.

Focused native13/13 passed (no skips), run
`39ffd710-f24d-4cdc-8003-93d4bd91635f`. All seven storage compile-fail docs and
storage all-target Clippy with warnings denied passed on AppleARM. Original
baseline-onlyb3efed352fc192f308b03b65eb05d7a4746a0049 was consumed intact.
New three Unix identities/doc locations require owner5 additive support. Full
native/current-head gates, restart/recovery/relocation, an actual owned threaded
resource-transfer bridge and domain policy/SQL stay separate; Windows directory
durability/config mutation remain excluded. No compensation or import admission
authority follows from these generic file observations.

Full source-combined AppleARM cargo xtask check passed155 nextest/zero skips,
nine separate compile-fail docs, unchanged146 corpus/strict15/source19 and both
graph/fmt/build/Clippy/discovery policies, run
`dc3432d0-e20c-4283-bd36-59248762ff10`. Only this validation note was appended
afterward. This supplies no Windows execution, Native restart/recovery or full
effect-handoff/4.11 qualification; corrected/current provider gates remain open.

### Partial 4.9: read-only regular-child open relative to the real Windows lease

The next private safe wrapper uses the already approved NtCreateFile subset
solely with FILE_OPEN, NON_DIRECTORY_FILE, synchronous NONALERT/SYNCHRONIZE and
OPEN_REPARSE_POINT/OBJ_DONT_REPARSE. It cannot create, overwrite, rename, replace
or flush. One lossless UTF16 component has checked USHORT BYTE lengths and a
bounded allocation; separators, drive colon, NUL and dot traversal refuse before
lookup. The actual parent handle/ID comes only from the real LibraryLease, not
from a separately supplied File/path/ID tuple. The returned read-only object has
an explicit destructor retaining that lease through its actual File close.

Function-local reasoned unsafe expectations and per-call SAFETY comments cover
initialized/aligned exact SDK OBJECT_ATTRIBUTES/UNICODE_STRING/IO_STATUS_BLOCK,
live stable local pointers, synchronous final completion, NTSTATUS handling and
the sole successful raw-handle transfer into std File. No BOOL/GetLastError mix,
overlapped/event/APC/pending-pointer escape, raw-handle duplication, blanket
allowance or new feature/graph edge is introduced. Child type/link/reparse and
actual volume/128-bit identity are checked with the existing query wrapper.
The documented NtCreateFile contract provides final completion on return:
https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile
https://learn.microsoft.com/en-us/windows/win32/api/ntdef/ns-ntdef-_object_attributes

Windows lease revalidation now checks its known lock-file child through this
same anchored regular-file open, retaining original root/file pin comparisons.
It does not read through the actual held byte-range lock. Two new required
Windows-only source identities cover readable pin lifetime, exact Unicode/space/
percent names, actual lease revalidation, invalid/absolute/drive/traversal paths,
UTF16 byte bounds and lossless surrogate encoding, missing/wrong-type objects,
hardlink/reparse refusal and preserved foreign bytes. A missing name never creates
a file; denied symlink creation is failure, never a skip. Native compilation,
ABI/lifetime/success/failure/race execution and independent review remain pending.

This uses no DIRECTORY_FILE flag combination and grants no directory capability,
ancestry qualification, create-new/publication/config mutation, directory flush,
NTFS/locality predicate, NativeWindows activation or full4.9 claim. Actual Windows
execution is required; host tests exclude this target code. Original98bf additive
NativeFiles discovery support is preserved in ancestry without recreation.

Full host cargo xtask check passed155 nextest/zero skips, nine separate
compile-fail docs, unchanged146 corpus/strict15/source19 and both graph/quality
policies, run`17cef1d9-964f-4897-ae55-118d793a49ce`; only this validation note
was appended afterward. Windows-only code/tests were excluded by that host;
two new source-defined Windows identities still need additive discovery and
actual native compilation/execution. No old native result is carried forward.

Follow-up buffer review initializes IO_STATUS_BLOCK's union with its NULL
pointer member, covering its full pointer width rather than only the narrower
NTSTATUS member on x64. No union value is read in Rust, no API/control flow
changes, and no new unsafe scope is introduced. The existing host run excludes
this Windows-only refinement; actual native compilation/execution remains open.

### 4.6 / native-state prerequisite — short-lock profile admission

ROOT4-NATIVE-STATE-V1-757 requires real inflight ownership before freeze, rather
than holding a profile mutex across callbacks or filesystem effects. A move-only
StateReservation now takes the actual cached state under a short mutex and
restores it after synchronous work; peers receive ProfileBusy without a second
state owner or hidden retry. Unwinding poisons the retained generation. Existing
concurrent update/binding tests retain all snapshot, single-winner and losing-copy
ownership assertions and explicitly handle bounded refusal at the caller.

Apple ARM: all eight profile unit tests PASS (including the new callback/peer
admission race), all twelve profile integration tests and thirteen native tests
PASS; storage all-target Clippy with warnings denied PASS. No native restart,
state-record, clean finalizer or Windows qualification follows from this first
prerequisite. ORIGINAL 9c17 discovery and d6ea minimal helper graph supports are
preserved as direct ancestors; the helper remains a separate queued slice.

4.6 scope correction: the new real profile-reservation race test is explicitly
macOS/Linux, where native directory durability is implemented. Owner5's focused
ARM execution PASS1/1 exposed the source-only Windows import/success-assumption
gap before discovery publication. The test now follows the actual Unix route;
Windows continues to require honest DirectoryDurabilityUnavailable refusal.
No Windows compilation/execution or all-target test qualification is claimed.

### Partial 4.7: durable Unix activation and consuming clean closure

Release `ROOT4-NATIVE-STATE-V1-757` authorizes this private macOS/Linux slice.
Original profile-admission support `a8d79591ad025d69f6626ae1726879d4b0b9f917`
and helper graph support `d6ea6c1fd96cc54106cf6738c7b434df31c56f7e` remain
unchanged ancestors. No root/shared port, dependency, Go marker/profile-v1 wire,
SQL, preservation input or domain repository changes are part of this slice.

The one root-local `.librairii-native-state.json` records strict bounded private
v1 data: format, phase, epoch, exact absolute UTF8 namespace/data/library/source
paths, original library-ID spelling and lower-case marker/manifest SHA256s.
Unknown/duplicate fields, types, trailing values, invalid phase/epoch, overflow
and over64KiB refuse. ImportedClosed has epoch0; acknowledged activation uses
the checked next positive epoch; clean closure retains that epoch. Source and
initial manifest hashes remain historical provenance, never current DB hashes
or persisted inode/cleanup authority. Incoming manifests reject the reserved
record and exact `.librairii-native-state.json.tmp-` prefix before copying.
Only an actual created record FD extends the physically owned copy witness.

Binding owns a short-lock profile reservation through copy/record verification
and publication, without a mutex across callbacks, filesystem or SQL. The record
is create-new/no-follow/nlink1, retained, file-flushed, root-flushed and freshly
checked before profile authority publication. Before SQLite open/init/migration
or managed-directory mutation, next Active bytes must pass atomic replacement,
file/root flush and fresh ID/byte readback. Partial control allocation errors
retain the actual FD/name/partial bytes; every later transition failure retains
old/new/displaced objects and uncertainty. No prefix scan, Drop cleanup or
candidate-file-to-authority conversion exists.

`NativeLibrary::reopen` reacquires an existing checked root lease and actual
native namespace/binding, validates retained marker/record objects, raw ID,
profile relationship, recognized DB family through a disposable private probe,
sidecar absence, containment and source/data/native separation. ImportedClosed
requires the already durable complete/nonrecovering profile with exact root and
raw ID; an orphan cannot publish or retarget it. Active, corrupt/foreign records,
recovery journals, control leftovers, aliases and unexpected entries refuse
without cleanup. Closed candidates require all previously initialized managed
directories. This conservative first route also requires the historical source
path to remain materialized at its exact canonical location for separation;
it does not replay or mutate that source. Legitimate DB/archive/catalog edits
do not trigger comparison with the initial closed-copy DB hash.

`NativeLibrary::finish` consumes the real owner after DB/file borrows end. It
must own the actual frozen profile state, observe successful stop/drain/five
connection handshakes/reader+writer joins/checkpoint/close, retain no unacknowledged
bounded result slot or custody owner, and prove empty staging/trash plus no
foreign recovery/control artifacts. Retired recoverable trash is not settlement.
Already-admitted profile work returns Busy with the consuming owner, before
freeze/Closed writes. Every non-control settlement predicate holds BEFORE the
Active-to-Closed replacement, so possible late-persisted Closed bytes remain
truthful after response loss. Failure returns the actual frozen/fenced library,
namespace borrow and old/new record objects. No timeout, Drop, database reopen
or error return clears generation exclusion. Success drops root/file/record
resources and the root lease before releasing profile binding; the store's
namespace lease can outlive them. Abandoned Active owners always fence binding.

Required 4.3 acknowledgement tightening is included: a dropped native Pending
cannot certify closure, and a forgotten Pending still owns its bounded slot even
after worker execution joins. Repair/barrier remain available under the retained
generation. Failed worker startup/close cannot mint the positive native close
evidence. Generic isolated workers and their existing dropped-result behavior
remain unchanged.

The source-grounded ProfileBusy race was reproduced on the actual checkpoint:
two tests failed because an admitted language update's known pre-admission Busy
permanently fenced healthy binding. The same two passed after restricting the
exception to that boundary before queue admission or first Active/SQLite effects;
no callback ran. After the update actually settled, ordinary read/write and clean
closure succeeded. Changed/poisoned/uncertain/finalizing binding still fences;
critical readback is not a fence reset.

Finite AppleARM verification on these source bytes:

- `cargo test -p librairii-storage --test native_library --test profiles
  --test adoption_inputs --test database_workers --locked`: native22, profile12,
  adoption10 and worker11 PASS, zero skips/filters in the selected binaries.
- `cargo test -p librairii-storage --lib native:: --locked`: five new native
  codec/path/activation/Closed uncertainty cases PASS;43 intentionally filtered.
  The separate actual post-open partial-control allocation case PASS1/1.
- Eight storage compile-fail docs PASS, including consuming closure while a DB
  borrow is alive; storage all-target Clippy with warnings denied, fmt and
  diff-check PASS. Existing Files/guard/stage/receipt DB-borrow docs are retained.

New source identities are one all-target manifest case, fifteen macOS/Linux
native/control cases and one Linux-only actual invalid-UTF8 owned-copy case.
The host filesystem refused materializing that invalid name before an owner
could exist; this failed assumption is retained, not counted as a Mac copy test.
A separate macOS/Linux pure path test proves lossless-string refusal. Strict
record, real clean restart/next epoch, orphan refusal, profile-update/finalizer
race, changed controls/sidecars/links, failed close, actual committed-but-unread
result, abandoned Active, unsettled trash and boundary response-loss cases are
separate observations. Abandonment/response-loss injection is not forced-kill or
power-loss qualification. Actual other-host tests and source-defined discovery
UNION/current full quality remain pending owner5 support.

This supplies no domain settlement/recovery/import/jobs bridge, automatic Active
recovery, Windows restart/directory flush/config replacement, physical durability,
full4.7/4.11 or merge admission. Current-head native/provider/cumulative checks
and independent exact-head review remain required.

Finite historical provider evidence supplied independently by root at exact
`e858da251f7653f2158a6e09169750b8a82a52e8`: all eight checks SUCCESS;
Windows job118938962787/run39513095237 completed101 nextest/zero skips including
both relative-read cases and all four sharing/query cases; Intel job118938963502
SUCCESS. Native PowerShell/full quality/fresh autocrlf passed there. This closes
that exact read-only Nt child ABI/behavior observation, preserves the old65ff
sharing RED, and does not qualify the new native-state source or queued NTFS
helper, Windows durability, full4 or independent review.

4.7 reused-runtime regression: on actual3f source plus the new owned test,
`cargo test -p librairii-storage --test native_library --locked
native_same_owner_database_reopen_revalidates_active_record_before_sqlite
-- --exact --nocapture` failed1/1 (22 intentionally filtered). After real first
DB close/drop, BOTH an externally overwritten valid Closed(sameepoch) record and
corrupt record bytes admitted five initialized SQLite connections on the same
NativeLibrary. The test explicitly joined unexpected owners and retained the
changed record, unchanged DB bytes, root lease and consuming failure owner.
RED log SHA256 `dafde43bbff50a768cff930d23a9ecef2f8425e6d27ad44003a017022f4d1965`.
This is distinct from current discovery/native/provider evidence; the regression
commit deliberately retains the RED mechanism until its following correction.

The following scoped correction retains regression
`11c38cb9c701bd47b0fc817bd9f9fadbcda39ae0` unchanged. EVERY runtime claim now
freshly validates the retained record before the managed-directory fast path;
reuse additionally requires the in-memory phase Active. No epoch is reset or
Closed/corrupt disk state rewritten. A mismatch fences the actual owner before
SQLite/thread admission and retains the exact changed bytes/lease. The SAME
one test then passed1/1,22 intentionally filtered, on AppleARM; GREEN log SHA256
`6a40285f33d79fe7a1f750cb315789a66779441df771e93407e30c56e016f2fa`.
No full/native-other-host/provider rerun was used for this narrow assay. Queued
Windows helper source is excluded from these commits and this host execution.

Original aligned discovery support
`1e954664d06e225a76510e246a667333c0fa7bcb` is consumed by fast-forward. It
preserves original17-entry `d04d388e4826777059f8a8af1379fe8e5b727e67`,
RED11c38cb and correctionc787cb through the original owner5 merge. Net source
delta versus correctionc787 is baseline-only. Owner5's actual corrected-root
full quality PASS173/zero skips +10 workspace compile-fail docs (8storage),
146/strict15/source19 and both graphs is finite AppleARM evidence at1e954.
The prior cached xtask invocation used the unrelated C2 compile-time root and
stopped before native tests; that launcher/cache failure is retained separately.
Rebuilding only that owned cached package supplied the correct source root;
no product/policy/dependency waiver or C2 source substitution occurred.

### Partial 4.8: a readonly source from the actual retained managed parent

`SOURCE6-READ-ISSUER-QUAL` adds
`LibraryFiles::open_read_source(FileArea, EntryName) -> OwnedReadSource` on
macOS/Linux. It derives Archives/Catalog from actual retained managed descriptors;
there is no arbitrary Path/File/metadata/TestRoots constructor or general external
picker permission. The owner retains readonly File + parent FD + exact raw name,
original file/parent identity, nlink1, size and mtime/ctime observations. The
existing safe openat route fixes RDONLY/NOFOLLOW/NONBLOCK/CLOEXEC and checks the
regular object and name-to-FD identity. No new dependency, unsafe, shared port,
crate wiring or cleanup/publication/activation conversion is introduced.

Read/Seek use the SAME FD. Revalidation detects substituted parents/names, aliases
and observed size/change differences while preserving the actual owner. New
issuance requires ordinary intake and a healthy generation. Already-retained
readonly identity/name observations can drain after intake stop or uncertainty;
they never clear exclusion or authorize new effects. These cooperative metadata
and EOF/hash observations do NOT establish adversarial source immutability or
snapshot exclusion. An initial size is data, not an immutable/product byte cap.

The source physically borrows Files, which retains the actual SAME Database,
NativeLibrary/profile/root leases or the actual isolated library. Three new
compile-fail cases cover DB close, Files drop followed by close, and the absence
of Write/writable File extraction. No nominal ID/path can replace those borrows.

Actual scoped AppleARM: four `owned_read_source` unit cases PASS (48 intentionally
filtered); the exact native source-after-stop/real-COMMIT-uncertainty integration
PASS1/1 (23 intentionally filtered). It reuses the admitted copied archive bytes,
retains fresh writer readback and an actual fenced consuming finalizer failure.
All eleven storage compile-fail docs and storage all-target Clippy with warnings
denied PASS; rustfmt/diff-check PASS. Readonly FD access, exact Unicode/raw name,
alias/parent/name substitution, observed growth and retained bytes are separate
checks. A sixth source-defined Linux-only actual non-UTF8 basename case is not
executed on this host. No skip, actual Linux/Windows, full workspace quality or
current provider qualification is inferred.

This read-only constructor does NOT repair or qualify create_stage allocation
failures: its bare Err may still leave partial private bytes. The first future
copy proof starts with an actually successful stage/guard and the source owner.
No section5 code/API handoff, external-picker permission, import inspection/SQL,
resource-budget decision, source cleanup, custody transfer or production bridge
is released by this mechanism. Owner5 additive discovery/current quality and
actual target evidence remain required.

### Partial 4.9: finite fixed-NTFS handle-observation helper

`QUAL4.9-FIXED-NTFS-757` adds a private readonly helper using the already
consumed exact windows-sys0.61.2 graph plus originald6ea WindowsProgramming
feature. On the SAME retained File, it requires exact complete NTFS name,
GetFinalPathNameByHandleW OPENED|GUID output with checked `0<len<capacity` and
NUL, a strictly validated GUID root/trailing backslash, and positive published
DRIVE_FIXED from GetDriveTypeW only on that derived root. Fixed initialized
16/1024 WCHAR buffers refuse truncation without growing/retrying; these are
qualification observation bounds, not product path limits. Facts/filesystem/GUID
are re-observed around the non-atomic calls. Unsupported/error/unknown/remote/
other classes or changed facts produce Unknown/refusal, with native causes
preserved, never a caller-drive or clear-remote-bit fallback.

Three individual safe native wrappers document their necessary local unsafe
calls: live std File/borrowed handle, aligned initialized fixed outputs and NULL
optional outputs, checked lengths, immutable validated NUL root, synchronous
completion and no pointer/ownership escape. No blanket unsafe allowance exists.
A function-specific non-test dead-code expectation keeps this finite helper out
of production preconditions while required native qualification is pending.

Three Windows-only required source test identities assert actual owned-file and
directory positives (Unknown is FAIL), actual short-buffer native failure and
truncation, and separately injected unsupported/changed/malformed observations.
The latter source controls are not execution on a real remote/removable/foreign
filesystem. There is NO local Windows compile/ABI/helper execution claim; host
storage Clippy excludes this target code. Rustfmt/source/diff review only applies
here, with owner5 additive discovery and exact-head native provider evidence
still required. The earlier exacte858 read-child pass does not qualify this code.

API success would establish only current OS-reported fixed NTFS on that host,
not physical/permanent storage, a whole snapshot, source custody, parent access,
both-parent rename persistence or power-loss durability. No directory flush,
NtFlush, ReplaceFileW/POSIX mutation, restart, activation or full4.9 grant follows.
The independently verified Unix read-source issuer is separately committed as
`34dc8e0ab2691f7bda0b49f8bb0fe65746da019a` and adds no Windows authority.

Original nine-identity support
`44c2416ab5d79c5513dd039f02e3f866972b8067` was fast-forward consumed and
published, retaining all prior objects/scopes and issuer34dc/helper8223 source.
ONE exact44 worker6 full quality run passed178 nextest/zero skips and thirteen
workspace compile-fail docs (eleven storage), unchanged146/strict15/source19,
both graph policies, discovery/fmt/build/Clippy. The launcher explicitly reported
Source44 and the actual worker6 root. Log SHA256
`e6e741698e4f38a58a06776a1a256caae325e56c8095c4ee0e8efaadbb7bdd9b`.
This historical AppleARM source-combined result does not qualify Windows helper
execution or the subsequently added process tests.

### Partial 4.7: actual process death and relocation refusal

Two macOS/Linux integration cases add finite lifecycle evidence without changing
any production mechanism, format, API, dependency or cross-section input pin.
The process case uses one actual child test process and explicit private owned
fixture locations. The child independently verifies the admitted closed source,
creates its OWN new copy and marked native profile through actual constructors,
and opens the actual five-connection Native Database before readiness. No parent
copy receipt, path/ID tuple or TestRoots boolean is cast into child runtime
authority. The readiness file is test synchronization data outside native
namespaces, never activation/closure authority.

The parent verifies both kernel locks are held, forcibly kills and truly waits
for the child, then verifies the root/profile locks become available. That OS
release does NOT permit mutable restart: actual checked reopen refuses persisted
Active, preserves exact record bytes, source DB, legacy settings and all managed
archive bytes. No child Drop/finalizer/clean flag executes and no journal/sidecar
cleanup or Active recovery is attempted. A readiness deadline fails the test and
the owning host fallback kills/WAITS rather than inventing a closed/durable pass.
This is process-death evidence, not physical power-loss durability.

The relocation case first positively closes/drains/finishes the actual native
owner, moves its now-Closed root inside the owned data namespace, and verifies
old profile authority cannot reopen or be publicly retargeted from data. Exact
profile, marker and record bytes remain unchanged and no old path is recreated.
It proves refusal/preservation, NOT successful verified cross-OS rebinding or
automatic relocation repair.

AppleARM scoped execution: each new exact case passed1/1 with25 other native
tests intentionally filtered; storage all-target Clippy with warnings denied,
rustfmt and diff-check passed. No fabricated executed RED, full178 repeat,
Go/export/scalar/C2 campaign, actual Linux/Windows or provider qualification is
claimed. Current source-defined test identities require owner5 additive support;
current native/provider/cumulative/review and full4.7/4.10/4.11 gates remain open.
The distinct proposed Native terminal proof stays pinned to1e954 and the import
proof to44; neither is silently updated by these tests.

### 4.1 correction: Go UUID wrapper spelling regression

Frozen `preferences.Identity` trims the marker text before google/uuid v1.6.0
`Parse`. Its 38-byte form validates the middle36 UUID bytes without requiring
braces. The pinned shared Rust UUID parser requires braces in that form. Upper
URN casing already agrees and is not a defect. This regression retains exact
UTF-8 marker bytes/spelling and the unchanged shared-core grammar; it also
extends the existing private native-record codec test with exact wrapper
roundtrips. No upstream implementation/data, dependency, fixture, shared type,
format field or notice is copied or changed.

Actual AppleARM RED on the unchanged production implementation: the new marker
probe case fails `InvalidIdentity`; the existing native-codec case extended with
the same accepted spelling fails `RecoveryBindingRequired`. Nextest run
`2e6ec78e-426d-4ccc-9f9f-1d2ab0f41239`: two selected cases, zero passed/two
failed,155 intentionally filtered. This is an actual storage-boundary failure,
not a new Go campaign or a parser-parity/native-admission claim. Existing supplied
frozen-Go optional/text observations are supporting evidence only. Native N3
input1e954 and import input44 remain unchanged; no temporary proof is shipped.

The original failing regression is
`ce69e4b2e1927de878be30257cde42717aebe522`. The correction uses a private
already-trimmed storage spelling adapter: for byte length38 it checks the UTF-8
slice boundary and delegates the middle36 to the existing LibraryId parser;
other supported shapes still delegate unchanged. Both marker parsing and the
private native record use this one adapter. Raw spelling, exact marker bytes,
profile equality, canonical diagnostics and shared/core string parsing remain
separate. Surrounding native-record whitespace and malformed lengths/UUIDs still
refuse. The existing UTF-8 marker domain is unchanged; no broad arbitrary-byte
Go parser claim is made.

Actual AppleARM GREEN: SAME two selected cases pass,155 intentionally filtered,
run `4c8ae107-91b9-46e1-b21c-841b00fd0ce3`. The marker table now completes all
seven supported spelling cases including arbitrary and NUL wrappers, followed by
non-destructive malformed-middle/length/multibyte-boundary checks. The native
codec roundtrips exact wrapper strings while retaining all strict invalid-input
checks. Storage all-target Clippy with warnings denied, rustfmt and diff-check
pass. RED log SHA256
`280489cef1d6f4e21fa2775a5f6f9bfa1fda481856925da136ef9813b5db35f0`;
GREEN `734f985d7d704b255ffa528d1ff2022c25717cdaf3815953b0a19e0a17b27adc`.
No Go, full workspace, native-other-host, whole Native restart campaign or fixed
temporary proof was rerun. Owner5 actual additive discovery/current combined
quality and final provider/review/admission remain required.

### 4.8 correction: retain partial stage creation

The current stage factory can create a private directory/payload and then
return a bare error after a descriptor has been acquired. The original focused
regression injects an error after opening the actual payload, writes and verifies
its retained inode and bytes, and requires the unresolved generation fence.
Actual AppleARM RED: run `c744a2f2-e029-437c-b64d-2d21f4d99da5`, one selected
case failed because the fence was absent;157 other tests were intentionally
filtered. This is an executed post-acquisition failure, not a claim that every
acquisition/validation branch or partial-allocation cleanup is qualified.

The bounded correction replaces the bare-error stage factory with an owning
failure result; partial allocation resolution remains outside this slice. The
fixed successful-stage-input import proof and native/scoped terminal proofs are
unchanged. The separately assigned current-head frontend failure remains open;
no retry or frontend changes are part of this correction.

The ORIGINAL RED is `79a7c54` (log SHA256
`c7e35754865ca524bb3e299be36be8c9d8da32c31d53da4822e03b5dc49428f9`).
The correction REPLACES `LibraryFiles::create_stage`, in the existing public
Unix namespace, with
`Result<OwnedStage<'files, 'library>, Box<StageCreateFailure<'files, 'library>>>`.
There is no legacy bare-error adapter, `From` conversion, owned-cause extractor
or standard-error conversion on this borrowed failure. `cause()` borrows the
original structured error; `disposition()` distinguishes this call's positively
`NoAllocation` from `RetainedUnresolved`; `retained_allocation()` borrows its
opaque actual partial owner. Optional identities remain absent until observed.
Positive creation flags never turn a failed/unknown call into no-effects proof.

Every attempted allocation starts with the SAME Files borrow/original staging
parent and exact validated single components. Every returned directory/payload
FD is installed into that state before metadata, type, link, identity, name or
root/guard validation. This path does not use the bare-error child/open helpers.
Intake/generation checks precede each attempt and complete authority is issued
only after fresh final parent/name/link/root checks. EEXIST collisions confer no
ownership and are bounded to64; generic syscall failures remain retained and
fenced. No partial cleanup, retry, path scan, destructive Drop, new graph/export,
unsafe code or domain implementation is added. Only fully validated OwnedStage
can use existing retirement. Existing successful fixtures explicitly assert
success; negative cases retain the actual failure, with their source/guard
owners remaining outside the factory.

Actual AppleARM final focused run `202349fb-66d4-43b4-acd3-993d34af1848`:
20PASS/0skip,144 intentionally filtered. Seven new creation cases cover the
original RED,12 post-effect/acquisition/pre-validation checkpoints, known
pre-effect/no-create/collision controls, missing-FD syscall failures, actual
directory/payload substitution, a hardlink change after metadata, and the actual
Native close/finalizer boundary. Retained FDs/IDs and prefix bytes are checked;
later NoAllocation and owner drops preserve the prior sticky fence. The Native
case uses a verified corpus copy and SAME Database, performs mandatory writer
readback, drains/closes/drops DB, then retains actual NativeCloseFailure with
Active record/root lock/profile exclusion and unchanged partial bytes. Ending
negative fixture borrows is explicitly not cleanup or settlement. Existing
affected publication/reuse/retirement/cancellation/native-file cases pass too.

Storage doctests14PASS:12 compile-fail lifetime cases plus two compile-pass
controls. Direct rustc diagnostic checks on the actual boxed-failure example
produce E0505+E0502 (Files/DB) and E0502 (DB-only), with no E0277 conversion error.
The publication compile-pass control consumes the fixture Result before testing
receipt drain, so a leftover Result cannot masquerade as receipt lifetime proof.
Storage all-target Clippy with warnings denied, rustfmt and diff-check pass.
The first new Native negative fixture reached its final lock assertion but
looked for the library-lock filename in the profile namespace; that fixture-only
NotFound failure is retained separately and corrected to the actual public
ProfileInUse check. No product RED is invented from that setup error.

These seven test identities are Mac/Linux scoped. No Linux/Windows execution,
current whole-workspace/provider/admission, full4.8/4.11, partial-allocation
resolution, fixed-G/N3/C2/C70 replay or new Go/normalization campaign is claimed.
Owner5 actual additive discovery and then current combined quality remain next;
the independent current-head frontend failure remains separately unresolved.
