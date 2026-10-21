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

## 6.4 — Cancellation and durable boundaries

Cancellation wakes matching executing/queued tokens promptly and records the
request through the reserved writer lane. Queued items never enter the effect
adapter after cancellation. Returned committed results are retained even when
the token is cancelled. The adapter explicitly distinguishes BeforeCommit,
Committed and Compensating; cancellation is never treated as evidence of rollback.

Compensating execution retains its actual owner and retries restoration without
a cancellation token before persisting its terminal report. Reporting and final
resource settlement are also uncancelled. Typed phases expose running, entering
commit, compensation, reporting and settlement to later native presentation.
Metadata activation/report and removal restoration use this same contract;
Task 13 still owns the removal domain implementation and Task 15 its UI.
The test performs real file writes/restoration, holds compensation unavailable
across multiple attempts, verifies SQLite remains running until restoration,
and proves cancellation preserves the committed first item while queued items
are cancelled. Queued cancellation uses the frozen kind-specific outcome text.

## 6.5 — Terminal retries and continued reconciliation

Terminal reporting makes three attempts separated by 10 ms. An unknown result
requires fresh reserved same-writer readback before another original write.
Three failed attempts/readbacks cancel matching work and change the durable
reporting intent to `persistence_failed` interruption. Interruption gets its own
three quick attempts, then continues at one-second intervals without a shutdown
or cancellation deadline. Original reports and actual resources remain owned.

An interrupted acknowledgement explicitly differs from acknowledgement of the
original item result. Settlement receives both the original effect fact and the
actual durable interrupted header; it cannot infer rollback or delete a completed
export from the history status. The SQL interruption preserves already terminal
items. Queued observations whose parent was interrupted never execute effects.
Metadata remains reserved while its original owner is still settling, even after
the interrupted SQL state releases the schema's active-operation unique index.

Tests use actual SQLite abort triggers and timed attempt observations to hold
completion/interruption unavailable, then restore interruption and verify the
saved result and retained metadata reservation. A second test conservatively
loses a real commit acknowledgement, refuses two readbacks, and finally crosses
the real writer barrier: the counter increments once and completion is not
repeated. Busy intake tests retain and retry the same move-only command.

## 6.6 — Native startup and owned temporary recovery

Normal reopen still refuses Active control records. The separate recovery route
validates the existing complete native profile, raw library marker, native control
record, independent-root separation, actual directories and exclusive OS lease.
It verifies regular SQLite sidecars through an independent probe, persists the
next Active epoch before opening SQLite, then reconciles before returning an
admitted Database. Missing known managed directories at the pre-SQLite crash
boundary can be created under the retained root owner. Foreign/corrupt/aliased
control or roots remain refused.

The native control record now has an optional bounded temporary registry (64
entries within its existing 64 KiB wire budget); migrations 001–018 are unchanged.
Registrations require an actual retained Stage/native Files/Custody owner and a
fresh writer-validated running operation/item. They bind the complete immutable
SQL scope, actual parent/private-directory/payload identities and destination.
Export allocation first durably reserves its private destination/name, then
records the acquired owners before payload writes. An interrupted allocation
without sufficient ownership evidence stays in explicit recovery. Public scope,
marker, pathname or inode observations alone cannot invoke this cleanup route.

Startup removes only registered no-follow regular single-link payloads in their
owned private workspace, flushes deletion, then retires the control registration.
It never receives final export filenames as cleanup authority. Missing payloads
from completed publication leave final files intact. Substituted files, links,
unregistered staging, unexpected entries or changed destinations are preserved
and block new work with a recoverable diagnostic. Active nonresumable SQL rows
become Interrupted, preserving terminal items, outcomes and byte counts; cleanup
failure remains explicit. Existing catalog-specific recovery is retained.

Runtime startup owns a unique actual Database job-session reservation. Intake
waits until reconciliation completes, preventing duplicate sessions from starting
or interrupting one another. Shutdown repeats registered-temporary settlement
after all effect owners have drained. A real child-process kill test covers
registered import staging, partial/completed exports and preserved terminal item
outcomes. Normal recovery cleans only registered partials; symlink/hardlink
substitution remains untouched with Active recovery state. Completed exports,
unregistered destination files and the original legacy source remain unchanged.
All 29 native-owner tests pass, retaining every prior restart/fence assertion.

## 6.7 — History, active/latest and destination handover

History resolves the raw before-ID and reads its 21-row lookahead plus item
snapshots in one coherent writer-barrier transaction. Pages contain 20 operations;
newer inserts cannot shift the next page. IDs/timestamps are compared as stored
text, with no UUID/date normalization. Missing cursors return invalid-request.
Active views retain every active row (no History20 cap), then append one latest
terminal export using SQL COALESCE's NULL-only fallback. Both views enforce a
64 MiB retained-result budget and return an explicit error rather than a partial
or silently truncated page. Item outcomes and legacy source aliases are preserved.

Export-directory handover starts from a real saved terminal export, validates an
absolute existing canonical directory and retains its actual descriptor plus the
Database borrow. Immediate reveal revalidation checks immutable SQL scope and the
same directory object. Relative/blank/NUL/missing/file/symlink destinations and
substituted directories are rejected. The native UI/platform file-manager launch
is Task 15; this handover provides one validated path argument, never shell text
or publication/deletion authority. Tests cover concurrent inserts, tie ordering,
20-row cursors, 30 active rows, NULL versus empty finished timestamps, historical
outcomes/aliases and unsafe/stale destinations.
