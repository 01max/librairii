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
|4.2|Exact migrations001-018, schema identity and contiguous ledger|Implemented/host verified `ea8e60f415b5ea23fb50f8899594884ef13b6c8f`. Setup `e6e527af79a21149665670a36171ef5d701be05c`; owner5 graph `bf720718c4736e0ecf5f6ded947ccbce49f1ba16`|
|4.3|One owned writer plus four reads, bounded jobs/transactions and close|Implemented/host verified; source SHA recorded in following commit|
|4.4|All18 admitted populated prefixes, full schema/rows/query outcomes and actual Collection-open projection; no fixture-specific patches or eager DB-open rewrite|Pending|
|4.5|Online WAL backup, integrity/FK, rollback and foreign DB+sidecars preservation|Pending|
|4.6|Strict64KiB v1 profiles, atomic serialized updates and isolated native namespace|Pending|
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
the new4.2/4.3 identities remains serialized through owner5 before admission.
