# Section 4 storage compatibility evidence

Scope: generic storage only. Import and operation/history domain repositories,
their reserved tests, root graph/shared ports and Go bridge17 are excluded.
Release: `ROOT4-4ea2caf`; approved base
`4ea2caf71bbe61ed996942daa9005de05b75f9da` (parents9a+reviewede60;
tree `de6034c706dac0c5e8a1f3aa68a8a9fd565015fd`). Root's release activates
owner5's prepared initial storage-shell handoff. ActualM provider checks are
pending at implementation start; no all-green or native-storage claim follows.

Implementation proceeds in numbered order with a dedicated Conventional Commit
and `OpenSpec-Task: 4.N` trailer for each subtask; fixes remain separate. Exact
completed SHAs are added in a following commit, so no commit claims its own SHA.
Owner2 alone updates canonical OpenSpec checkboxes. This ledger is shipping
evidence, not an additional checklist or acceptance of unfinished work.

| Task | Implementation/verification work | State / commit |
| --- | --- | --- |
|4.1|Explicit fresh isolated root; exact layout, managed paths, raw identity; reject escape/invalid/nonfresh inputs without profile discovery|Implemented/host verified; source SHA recorded in following commit. Setup `fcf467f41a39c3670a38db063a5304a60f6e93d1`|
|4.2|Exact migrations001-018, schema identity and contiguous ledger|Pending|
|4.3|One owned writer plus four reads, bounded jobs/transactions and close|Pending|
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
