# Task 12: exports, trash custody and diagnostics

Base: `7883a73d3a991ad50d174f2c456365aad029d039` (Task 11 merged).
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
