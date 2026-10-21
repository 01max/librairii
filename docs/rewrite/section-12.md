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
