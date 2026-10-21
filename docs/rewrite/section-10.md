# Task 10: curated Shelves and explicit reviews

Base: `31f23b9859500de415a72d930e258866a279bab1` (Task 9 merged).
One PR targets `rewrite-in-rust-native-desktop`, with a commit per 10.1–10.11.
Linux x86-64 and macOS ARM/Intel are the admission targets; Windows is deferred.

Shelf participants use the existing owned SQLite writer and unchanged migrations
001–018. UUIDs, not numeric story IDs, own approved membership. Duplication copies
raw order, retention, cached titles, exclusions, source history and origin times
with independent source IDs. Reads and library changes never curate membership.
No protection setter is exposed. Ordinary removal and removal with exclusion are
separate commands; clearing an exclusion does not add anything.

Creation, updates, source changes and linked Collection deletion have opaque,
clonable preview plans bound to their writer generation. Private, kind/target-bound
tickets expire after 15 monotonic minutes; all kinds share a 64-ticket oldest-first
cap. The writer claims a ticket once, reevaluates the complete relevant semantic
snapshot and validates all decisions before writing. The snapshot includes raw
Collection definitions, current UUID matches, approved state, exclusions,
availability and displayed candidate titles, with Broken flags and numeric story
IDs excluded. Even no-op applies consume their ticket. A failed/rolled-back or
uncertain attempt also requires refresh, conservatively preventing retries from
reusing authority after an unobserved outcome. Dropped pending results retain the
existing runtime settlement/fencing behavior.

Unknown source coverage suppresses all source removals while permitting healthy
additions. Detachment requires Keep or explicit removal/exclusion for every
affected member; unavailable/unknown coverage permits retention only. Last-source
detachment retains every survivor manually. Linked deletion revalidates all
Shelves and commits their decisions, detachment and the guarded Collection
participant in one transaction. Its committed receipt reuses Task 9 navigation
handling. Historical source names remain after detachment/deletion.

Creation/update/source-change candidate and warning access is paged at 25 rows;
member and exclusion result pages also cap at 25. Counts cover all rows. Collection
deletion exposes one affected Shelf at a time. Explicit bulk decision helpers
operate on the full candidate set; previews have no default approvals.

Mismatch dismissal is a native profile preference in `shelf-warnings.json`, using
the same anchored atomic publication, leases and uncertainty handling as native
navigation. The versioned, bounded document is library-bound and preserves unknown
versions/corrupt files. Acknowledgements bind to semantic snapshot hashes; changed
coverage prompts again and Broken-only changes do not. Dismissal never records a
membership decision or authorizes apply. This separate profile preference extends
the frozen UI's session-only suppression to satisfy 10.8; SQLite stays compatible.

Evidence:

- Seventeen storage integration cases cover CRUD, decisions, overlap/provenance,
  reimport, incomplete coverage, source changes, linked deletion rollback, ticket
  eviction/replay/cross-runtime use, preferences, paging, and frozen Go parity.
- Ticket unit tests use explicit monotonic instants for exact 15-minute expiry,
  kind/target mismatch, oldest eviction, expiry pruning and replay.
- The v14 fixture upgrades via the real writer to schema 18, lazily opens its
  original Collection ID, preserves migrated Broken/user tags, exercises real
  tag commands and synthetic catalog activation, rejects stale reviews, Keeps,
  becomes unavailable, restarts and reconnects the UUID under ID 101. Its
  synthetic unfinished removal intent is explicitly settled as test setup;
  real removal/custody recovery remains Task 12. Original fixture bytes stay intact.
- The 5,000-story overlapping-source test repeats previews, applies 50 decisions,
  checks remaining 4,950 suggestions, provenance and all bounded models. Initial
  local worst preview/apply were approximately 442/443 ms; each enforces 5 s.
- `tests/fixtures/shelves/README.md` records actual frozen-Go capture/reproduction.
  Two captures are byte-identical; the unchanged Go Shelf/Collection suites pass.
  Rust compares every captured summary, decision and provenance field.

Task 10.11 includes independent Standards and Spec reviews against the pinned base,
followed by corrections and the full native quality gate/provider CI. Concrete
review and final-head receipts are recorded in the PR. This section supplies
headless domain/storage ports; Task 15 still owns native Shelf screens and focus/
keyboard behavior. It does not complete earlier deferred foundations, metadata
transport (Task 11), import/trash execution or the GUI/package cutover.

Independent review at `737a6a0`: Spec reported no actionable gaps. Standards
identified one P2: domain/command identities used raw integers and strings despite
the design requiring newtypes. The correction uses ShelfId/CollectionId in
records and command ports, StoryUuid in manual creation/addition and parsed StoryReference values for existing curation/review decisions, and
explicit SQL-boundary conversion. Read projections retain lossless stored UUID
spelling with a parsed identity accessor. Compile-fail examples reject a
CollectionId passed to a Shelf command and arbitrary strings used as UUID commands.
All seventeen Shelf tests still match the frozen reference after this correction.
Standards also suggested an optional private authority-record extraction (P3);
the four current plan paths consistently check the same invariants, so that
refinement is deferred without weakening their checks.

A follow-up Spec review caught a P2 regression in the first typed-UUID correction:
canonical decision text could no longer target preserved uppercase UUID rows.
The separate correction introduces StoryReference: parsed StoryUuid identity plus
exact stored spelling. Reviewed actions and existing-member commands now retain
that spelling; new manual seeds/additions still require canonical text admission.
The uppercase regression covers source Add, update Keep/removal, detachment
Keep/removal and clearing the resulting exclusion without retargeting a canonical
alias. Frozen state comparisons remain unchanged.
