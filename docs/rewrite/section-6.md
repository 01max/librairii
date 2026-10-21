# Task 6: jobs, recovery, history and shutdown

Base: `7883a73d3a991ad50d174f2c456365aad029d039` (Task 11 merged).
One dedicated PR targets `rewrite-in-rust-native-desktop`, with a separate
Conventional Commit for each implemented subtask. Linux x86-64 and macOS
ARM/Intel are the supported admission targets; Windows remains deferred.

## 6.1 — Durable operation state and compatibility

Creation and item-start transitions use the admitted Database writer. Item
completion and its parent counter update share one guarded transaction. Finish
requires every item to be terminal. Interruption preserves terminal items and
recorded bytes, cancels unfinished items and saves its reason atomically.
Acknowledgement readback uses the reserved same-writer barrier and compares the
entire immutable command scope and payload. Refusal, rollback, acknowledged
commit and unknown outcome remain distinct; a later refusal cannot erase an
earlier unknown outcome. Public snapshot values convey observations, not file
custody or activation authority.

This ports the retained borrowed terminal interface from original reference
`8d91ba2f` and corrected signed-ID SQL implementation from `c2eb8ec6` onto the
current integration base. Those references are unchanged; no proof-union branch
was merged. Creation, exact Go JSON scope escaping, start transitions and legacy
presentation envelopes are additional production repository methods. Migrations
001–018 stay unchanged.

Raw legacy operation IDs, signed persisted item IDs, nullable outcomes and
immutable resolved story identities survive reads and reopen. Historical
`shelf`/`shelves` sources retain Collection interpretation and both presentation
aliases; curated Shelves remain separate. Absolute destination and managed-file
internals are omitted from presentation. Input validation uses the frozen
whitespace policy and a 16 MiB retained-command budget, not an export item cap.

Validation includes the preserved terminal port and 14 SQLite tests plus real
creation/start/completion/readback/reopen, legacy dynamic/curated source JSON,
invalid creation rollback and contradictory header readback tests. The private
controlled jobs engine remains historical model evidence, not production effect
composition.

## 6.2 — Owned bounded job session

The public application session borrows a real `JobStore` and effect adapter.
Its native implementation borrows the admitted Database, loads one individual
item through the reserved writer barrier and preserves legacy FIFO order. The
session owns two blocking workers by default (configurable 1–8), a shared ready
queue of 2W, at most W executing/settling owners and one source/admission slot.
SQLite stores the rest of each operation; the queue never retains a whole scope
vector. Intake retains one bounded 16 MiB command and one unread acknowledgement;
Busy returns the original move-only input. Effect adapters must independently
bound their own archive/network/file resources. Count bounds are not a claim
that arbitrary adapter-owned resources have a byte bound.

The import picker accepts at most 1,000 entries. Exports have no item-count cap;
the retained-command byte budget still applies. Metadata conflicts are checked
inside creation's writer transaction and backed by migration 009's existing
unique index. The reservation remains until a durable terminal operation state.

Scoped OS blocking threads are deliberate: they retain actual non-static
Database and managed-file borrows across the native event loop, then join before
those owners can close. Tokio remains the isolated HTTP runtime; no unbounded
spawn_blocking queue or fabricated static storage handle is introduced. Native
UI composition is Task 15. The runtime test uses a real independently adopted
native library, admitted Database and actual OwnedReadSource retained through
SQL completion and settlement, followed by clean Database/native closure.

## 6.3 — Coalesced progress and correlated observations

Each worker owns one scalar progress slot with monotonic byte/revision checks,
an opaque item epoch and a seal at execution completion. Late callbacks cannot
write into a later item or reopen terminal presentation. One dirty bit coalesces
wakeups; terminal truth is fetched from SQLite, never delivered through a lossy
progress queue. Export progress persists at 256 KiB increments or its final byte
through the reserved writer lane. Progress writes cannot update terminal rows.

Read requests carry an opaque session identity and optional existing navigation
QueryStamp. Equal numeric generation values on a new session do not admit old
responses. Query-independent operation observations survive browser changes.
Operation projections bind one raw operation ID and retain a durable terminal
snapshot against delayed active responses. Native UI rendering remains Task 15.

Before extracting variable-length SQLite text, reads check each row against
16 MiB and full snapshots against a 64 MiB aggregate budget. Oversized historical
rows return a resource-limit error without rewriting or truncating stored data.
The test emits 10,000 updates, observes one latest slot, checks real persisted
export bytes and late callbacks, and rejects stale query/session responses and
an older active snapshot after terminal persistence. Another test retains an
oversized legacy outcome unchanged while refusing its materialization.
