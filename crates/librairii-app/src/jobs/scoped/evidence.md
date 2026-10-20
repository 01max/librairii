# Scoped terminal source (PARTIAL section 6)

Release: `SCOPED6-TERMINAL-H-R4-S298`. Source starts at original R4
`c2eb8ec60c33b003d2263cc30175db03df4180f3`, retaining D
`8d91ba2f9958853a367e4e46ae78e49deef44f76` and shipping e81
`e81de948825d839c39794aa6fc4a56c15658b622`. Only `app/src/jobs/**`
changes. D, core, storage, manifests, root exports and discovery stay unchanged.

`jobs::scoped::run` supplies a scoped, typed terminal-reporting facade using
`&(dyn OperationTerminalStore + Sync)`. The host keeps its library/database
outside the scope. Only the existing W workers and calling coordinator invoke
callbacks; each synchronous provider owns/waits its own Pending. No static
authority, unsafe Send/Sync implementation, pool, helper or production source
port is created. This is a partial 6.5 reporting/lifetime contribution using
the existing 6.2 worker bounds; no presentation/6.8 completion is claimed.

Each worker retains its original Request, typed execution result, independent
work error and full command witness outside callback catch boundaries. Source
or work unwind before a DB call is a work/no-admission observation; a store
unwind is unknown until exact fresh readback. Work is never replayed. A
Committed reply, including no view plus an independent cause, makes completion
acknowledgement sticky. A supplied Finish plan is used only after a fresh,
coherent matching view establishes eligibility. No aggregate outcome is derived
from counters. Different legitimate terminal truth is retained. Raw operation
IDs, generation, signed item IDs, nullable strings and immutable scope survive.

Intake stop, work cancellation and requested unresolved return are independent.
A requested return cannot overtake a blocked callback or scoped join. The
outcome contains actual retained unresolved owners and unstarted inputs; it is
not a library fence, lease, close receipt or permission to reactivate. The host
must retain its actual fence/resources on unresolved return, settle or explicitly
transfer them, then drain/join and exclusively close storage with leases last.
Only controlled unwinding is caught; process abort is not responsive cancellation
or product refusal evidence.

The queue is 2W, W is 1..8/default 2, and there is one prefetch: at most 3W+1
dispatched requests and W+1 callback owners. There is no start/admission ingress
or persisted cancel-control call in this slice. Source storage remains outside
the run and is not represented as a hidden scope/item vector in a handle. A
slot-derived fence suppresses later dispatch of the same raw operation and
generation while required reporting is pending. Critical storage capacity is
the existing SHARED four FIFO slots including unacknowledged results, not four
slots per worker. Refusal retains the same original owner and prior uncertainty.

Three initial writes use virtual observed ticks 0/10/20; interruption has its own
budget, then guarded reconciliation steps are at least 1000ms apart. Due times
use the actual callback-return tick, so jumps/blocked callbacks do not catch up.
Tick regression/overflow are rejected. The latest two formatted causes are
bounded to 1024 bytes each, independently of the original work error. Retained
command/witness/view string capacities are measured separately; two extra field
copies per slot are budgeted for the pinned R4 reply witness and DB work copy.
These are logical payload measures, not a bound on arbitrary provider allocation,
opaque original error internals, source arenas or full snapshot materialization.

## Executed source/model verification

- Pinned Rust 1.99.1 `cargo check -p librairii-app --locked`: PASS.
- `cargo test -p librairii-app --locked`: 56 PASS, zero ignored/filtered,
  including 17 new scoped cases and all 39 prior app/port cases. No app doctests.
  After test-only failure-cleanup guards were added, the scoped 17 passed again
  with the unchanged 39 prior cases intentionally filtered; production source
  was unchanged by that cleanup.
- `cargo clippy -p librairii-app --all-targets --locked -- -D warnings`: PASS.
- Local controlled cases cover borrowed non-static providers, exact raw/signed
  IDs, sticky commit with missing/refused view, positive fresh nonapplication,
  commit-response unwind, pre-DB source/work unwind, independent work errors,
  delivery unwind, actual different terminal truth, full witness mismatch,
  independent stop/cancel/pending joins, W8 with four controlled blocked calls
  and four refusals, 25 retained/queued/prefetch handles, separate retry budgets,
  observed-return pacing, overflow/regression and invalid supplied scope.

The first app-only lint attempt found a large enum/redundant closure and unused
test variant; these were corrected before publication. Controlled cases use an
in-memory supplied provider; none is real SQL, critical4 saturation or durability
qualification. Existing R4 SQL tests are byte-unchanged.

## Separate upcoming composition

Owner5's NEW TEMP composition will pin actual S2
`98bf463725cc81a88a910057c6db8fd3600b1968` (direct fbd33), original D/R4/H,
and original E export if unchanged (otherwise owner6 authors E2). Owner5 owns
all-parent actual discovery/UNION and the TEMP-only storage-to-app SQL host
harness. Real W1/W2/W8 SQL/report/readback/drain proof and cumulative quality are
pending that composition. No SQL, full xtask, Go, corpus regeneration, native or
provider check was run for H alone. Fixed C70/Sd710/E proof and held shipping
PR42/e81 remain separate and unchanged. No full 6.N or merge claim.

## R4 terminal-item conflict correction

R4 `policy::inspect` returns Conflict when either the parent OR the exact target
item is terminal, after immutable scope and ALL-sibling counter validation. H770
incorrectly required a terminal parent. A source-grounded controlled regression
uses an active Running parent, coherent 1/1 sibling counts and a terminal Failed
target with different story ID, byte count and nullable outcome. It exercises
both direct Attempt and unknown-ack Readback conflict responses. Actual saved
item/header bytes remain intact; the requested Complete is not acknowledged,
recounted or replayed, and no parent terminal state or Finish is invented.

The single regression against unchanged H770 driver was executed RED:
0 pass/1 fail/56 intentionally filtered, cargo exit101, assertion that the
coherent item-only conflict must resolve to saved truth. Working log
`/private/tmp/librairii-3-scoped-terminal-item-conflict-red.log`, SHA256
`d4e96d35fbe50e9c647618a153b325751137b2e4a399f7e3d1a3dc95a5d1258c`.
After the dedicated OR-terminal predicate correction, the SAME regression was
GREEN: 1 pass/0 fail/56 intentionally filtered; log
`/private/tmp/librairii-3-scoped-terminal-item-conflict-green.log`, SHA256
`193992009833ebb740017cf2d7eec0036786f2e585467f0bb7902c976b5fa013`.
App all-target Clippy with warnings denied and diff-check passed. This is one
bounded model regression (now 18 scoped identities), not actual SQL execution,
independent/native review or another owner56/full-quality campaign. The same
TEMP C2 composition must consume the original correction history before its
actual SQL host proof; R4/D/S2/E/E2 and held shipping remain unchanged.
