# Partial 6.5 — borrowed synchronous terminal SQL source

**First R is source only. No SQL test, build, discovery, native review or provider
qualification is claimed by this commit.** Its parent has the application port,
but does not contain the unadmitted storage mechanisms or the owner6 module export.
The twelve integration assertions below must execute on the exact pinned temporary
composition before any result is recorded as passed.

Release: `PORT6-SYNC-R-D8d91`. Only this directory and
`crates/librairii-storage/tests/operations_repository.rs` belong to this contribution.
No storage root export, manifest, lock, generic worker, schema, test baseline,
canonical checklist, admitted preservation input or PR42 source is changed.

## Provenance and contribution ledger

| Role | Exact source |
| --- | --- |
| Frozen behavior | `73a93303a9a44cf0a92eaf525331561ed8c04460` |
| Validated earlier partial head, kept stable | `e81de948825d839c39794aa6fc4a56c15658b622` |
| ORIGINAL owner5 declaration D, direct parent of R | `8d91ba2f9958853a367e4e46ae78e49deef44f76` |
| Published candidate S for temporary composition only | `d710fd226c8c0cbd41a192f89d7f2917dccab202` |
| Required S poison-close correction | `a6fd4677e3069cef0d59300da77ba17dc1ae6215` |
| R contribution | Conventional commit with `OpenSpec-Task: 6.5 (PARTIAL)`; its exact SHA/tree and paths are reported externally through AO after publication |
| Owner6 export E | Future owner-authored output bound to exact R and S/C0; absent at first R publication |

The Git history and external AO ledger preserve exact contribution SHAs without
a self-referential commit hash in this file. No rebase, squash or cherry-pick is
authorized. Later fixes require separate commits and honest head-specific results.

Frozen source anchors: `internal/operations/repository.go:414–576` supplies the
two-guard completion, terminal finish and transactional interruption rules;
`repository.go:573–699` supplies stored parent/item columns and row-ID item order.
The raw DTO contract intentionally retains NULL versus empty, including both live
`story_id` and historical `resolved_story_id`, rather than implementing Go's display
COALESCE, JSON envelope or host basename projection.

## Implemented source boundary

`TerminalRepository` borrows the existing `Database<'library>`. It implements the
actual synchronous `OperationTerminalStore`; it opens no connection or pool and
starts no threads. Snapshot reads parent and all ordered rows in one read
transaction. Terminal attempts use one reserved repair transaction. Before any
mutation the same writer reads immutable inputs, target payload and all-sibling
counts. Required readback uses `barrier_read`, one fresh coherent writer view after
earlier admitted writer jobs. It never uses a pooled snapshot as commit proof.

Completion requires both UPDATEs to affect exactly one row. A failure returns an
actual transaction-body SQL error, forcing rollback of the earlier mutation.
Target terminal state is a use-case precondition; current pending/running is the
SQL guard. Completion bytes need only satisfy schema bounds, and may be below
prior progress. Only live `story_id` changes (zero becomes NULL); outcomes use
command-specific NULLIF(empty). Resolved identity and export inputs never change.

Finish permits the four non-interrupted terminal states and stored zero/zero,
guards active/completed==total, and applies NULLIF(empty) only to its error fields.
Interrupt requires exact nonempty raw values without trimming; it preserves prior
terminal items and every byte count, cancels only pending/running items and then
updates the active parent to interrupted/completed==total in the same transaction.
The caller supplies exact planned finish text; no clock or time normalization runs.

Matching terminal payload is acknowledged without a second increment, including
when a concurrent sibling has advanced the parent. Immutable inputs and counts
over all siblings must agree. A different legitimate terminal result is returned
as actual conflict/truth. Missing rows, contradictory scope/counts and unavailable
readback remain unknown. Commit/rollback acknowledgement loss is not noncommit;
the adapter neither blindly retries nor clears the Database uncertainty fence.
A proven commit remains committed when a subsequent ordinary view read refuses
or fails, with its independent diagnostic retained.

The caller's original command/report/resources stay outside consuming queue
closures. One reply witness plus one DB-only work copy temporarily coexist;
`command_bytes` measures one such copy's boxes and actual string capacities.
Snapshot's row vector and all raw text have their separate `snapshot_bytes`
measurement. Neither function establishes a quota or a one-item/snapshot bound.
There is no hidden waiter, task, retry loop or repair queue. Reserved critical four
slots include unacknowledged results and are shared with barrier/backup work.
QueueFull is attempt-local, retaining the original caller owner and earlier
uncertainty. Accepted Pending is waited on synchronously by that same caller.

## Authored assertions — not executed at first publication

All twelve integration functions begin `terminal_sql_`. They use fresh TestRoots,
an actual IsolatedLibrary and its same Database, seed declared compatible rows
through that writer, then call the typed port. No production path/profile/endpoint,
copied oracle, fixture generator or fake persistence callback is used.

- Raw 36-character non-UUID identity, nullable/empty text, exact group JSON,
  Unicode/private text, historical identity and row-ID ordering; separate raw
  snapshot/command storage accounting.
- Atomic completion, exact stored NULL conversions, decreasing terminal byte
  count, immutable resolved inputs and idempotency after another sibling commits.
- An actual AFTER UPDATE SQLite trigger changes the parent after item mutation;
  both completion and interruption must roll back all trigger/item changes when
  the second guard affects zero rows.
- All four Finish targets and Interrupt on stored zero-item parents.
- Interruption retains prior terminal payload and all bytes, raw whitespace
  reason and planned time; actual terminal conflicts remain unchanged.
- Incomplete Finish, invalid typed target/byte range, wrong generation and empty
  interrupt code cannot fabricate acknowledgement or modify rows.
- Missing input, changed immutable scope and schema-valid but sibling-incoherent
  counters never acknowledge or imply rollback.
- A real deferred foreign-key COMMIT error produces uncertainty. Saturating four
  owned critical slots temporarily refuses readback without erasing it; a later
  fresh barrier proves actual nonapplication. Ordinary writes remain fenced.
- A real prior writer commit whose Pending has not been acknowledged is observed
  by the later writer barrier without a second completion increment.
- Real critical QueueFull consumes copied fields while the original move-only
  report/facts remain owned; variable command text is measured separately.
- Ordinary intake stops before Interrupt; required repair commits while its
  subsequent ordinary snapshot refuses. Its fact survives and reserved readback
  confirms it before database close; the actual library lease outlives DB joins.
- Two scoped callers complete distinct siblings through the same writer and
  receive sibling-coherent matching evidence.

The actual generic blocked-drain/poison-close tests belong to S and must remain
mandatory in C, especially the separate a6fd467 correction and 44185 regression.
R does not relabel clean-close or symbolic jobs tests as that qualification.

## Required temporary and shipping qualification

Owner5 creates C0 from exact R and history-preservingly composes exact S, resolving
only owned root/discovery policy. Owner6 then authors ORIGINAL E (storage module
export only) against actual C0/R/S. Owner5 derives actual cumulative nextest IDs
and target scopes from the combined executable tree, preserves BOTH parent test
inventories/strict Python/docs/pins, and runs the pinned full xtask once eligible.
Record exact D/R/S/E/C, effective tree/root Cargo/lock/features and actual results.

Passing C is composition-only evidence. C and unadmitted S must not be pushed or
merged into shipping PR42/43/global. Final shipping requires the actual admitted
storage/base or an explicitly authorized visible topology, owner5's final UNION,
and fresh exact-head checks/review/provider CI. No full 6.5/6.7/6.8/6.9 or section
checkbox, real threaded jobs bridge, startup cleanup, NativeLibrary activation,
history API, import/custody/reveal authority or PR merge is qualified here.
