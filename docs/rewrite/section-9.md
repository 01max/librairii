# Section 9: dynamic Collections

This task adds Collection domain/storage operations on the existing owned SQLite
workers and unchanged schema. The PR targets `rewrite-in-rust-native-desktop`;
Linux x86-64 and macOS ARM64/Intel remain the active targets. Windows is deferred.

Each subtask has its own commit:

- 9.1: CRUD, exact normalized Collection namespace, stable IDs/revisions,
  independent duplication, validated atomic order and lazy compatibility reuse.
- 9.2: current-state evaluation and coherent counts after library/tag/metadata
  changes, with Broken flags independent of membership and approved Shelf rows.
- 9.3: OR preview with per-source, unique and overlap counts in one transaction;
  SQL UNION counts all pages without retaining an uncapped story-ID set. Saving
  copies criteria rather than source IDs, subject to existing saved-query limits.
- 9.4: explicit query replacement, preserving invalid bytes until repair. Opaque
  plans bind the writer runtime and entire stored row, and revalidate live
  definition kinds/value ownership at apply. Removing a predicate is an explicit
  replacement; invalid queries never silently degrade into all-stories results.
- 9.5: deletion impact includes active and inactive Shelf sources. Unlinked
  deletion clears only historical source foreign keys, retaining approved rows,
  origins and exclusions. A committed deletion receipt can convert the current
  Collection view to custom while retaining its effective query, sort/page,
  selection and presentation; stale query stamps cannot move navigation.
- 9.6: frozen Go capture, all-scalar name comparison, invalid raw bytes,
  rollback/cancellation, live-reference races, legacy IDs and restart persistence,
  followed by independent Standards and Spec review.

`collections::source` is a read-only same-transaction participant for later
Shelf/export work. It deliberately decodes stored bytes directly and validates
current references, without lazy migration or attention writes. Inspection and
ordinary evaluation use the existing lazy compatibility path instead. Attention
is returned as committed domain data; SQL/worker/cancellation failures remain
separate transaction failures.

The deletion-impact function is data for Task 10's transaction boundary. Active
links are refused; Task 10.9 still owns reviewed, atomic detachment across all
linked Shelves, including member decisions and full review fingerprints. These
impact counts do not authorize detachment. The existing CRUD delete participant
is also guarded against active links.

The reference fixture README records reproducible frozen-Go capture commands,
exact source hashes, retained output fields and the digest framing. Routine
verification reads the capture without regenerating it or altering admitted
preservation fixtures. The full frozen Go Collection suite and application
Collection vertical slice pass, and an independent repeated capture is identical.

Verification uses `cargo xtask check`, the current three-host native CI policy,
and two independent reviews. This task does not complete deferred foundation
work, Rust import/metadata/export transport, curated Shelf workflows or the GUI
flows in Task 15. The local ignored OpenSpec repository retains its own checklist;
it is not committed, pushed or archived by this PR.
