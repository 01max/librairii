# Controlled headless jobs evidence

Release: `NEXT6-8b-HEADLESS`, actual handoff preparation
`e008da4fa60670daab84b900bbfe9591e0a8fc2a9c7a7050aac68d7302329b3d`.
This is partial section 6 work. Every full 6.N, 6.8 presentation and 6.9
durable/restart gate remains open. The private module has no production
composition, public shared ports, storage/import/custody effects or GUI.

## Implemented partial 6.2

- One coordinator and fixed 1–8 owned worker threads (default two); shared
  queue 2W, at most one fetched/reserved item, and at most 3W+1 item handles.
- One separate move-only ingress input. Busy/closed returns the same input
  before admission; an issued uncertain start stays retained until an explicit
  controlled readback response. Created records survive reply disconnection.
- At most W worker calls plus one coordinator call. Unacknowledged reports
  retain their worker slot; there are no per-operation producer/retry threads.
- Separate capacity measurements for ingress, item payload and report data.
  No work item captures the full input scope. Import 1–1000 and export beyond
  1000 are separately exercised; metadata reservation lasts through report ack.
- A bounded source cursor carries a finite-cycle watermark. Actual threaded
  offers retain per-operation order while new operations arrive.
- Stop is independent of work-queue capacity. Blocked calls keep joins pending;
  callbacks run outside the queue lock. Drop joins instead of detaching.

The callbacks are controlled test seams, not actual database acknowledgements
or custody receipts. Source data retained by the in-memory test backend is
reported separately from the bounded scheduler pipeline. No constant heap-byte
ceiling or real SQL fairness/recovery qualification is claimed.

## Verification before shared wiring

Apple ARM, pinned Rust 1.99.1. A nonshipping harness in the owner's AO working
directory imports these exact source files without editing app `src/lib.rs`.
Strict `clippy-driver` test and non-test compilation passed with warnings,
ordinary Clippy, unwrap/expect/panic/todo/unimplemented/debug macros denied.
All 11 controlled tests passed with ordinary test concurrency, zero ignored
or filtered. Saturation is measured at W=1,2,8; an export of 1117 items completes
without truncation and retains its original payload allocation.

Two initial driver synchronization assertions failed: the join observer had
not yet requested stop, and separate historical peaks did not prove simultaneous
pipeline saturation. The driver now requests stop synchronously and waits on
the combined handle peak; the assertions remain. These were corrected before
the first scoped commit. The standalone harness is not cumulative nextest or
provider evidence. Owner5's actual app export/discovery, merged repair-base
checks, final native review and current CI remain required.

The module's narrowly reasoned non-test `dead_code` expectation reflects its
intentionally private, uncomposed handoff. Tests compile without that expectation.
No required test, fixture, warning category unrelated to that composition gap,
or existing check is disabled.

## Commit custody

The original pure slice remains preserved, with its exact validation in PR42:

- `821a59a73b82010b2cee98c28bfb786dafce9587`
- `c90752e872738d2d7efee6d31e0f1b4a9cffa06d`
- `11e60b97bad8de1925fcb05b4b38a9156a3b9a23`
- `11bc9ffa53ecebab590a38df1aff4ea2d70516d3`
- `45e4560d159c695b0765fffa8dbbc450ef6a9c30`
- `8b1a421028799d2996cbc524371a817b536f67cd`

The partial 6.2 commit is identified by its `OpenSpec-Task: 6.2` trailer and the
external PR/AO ledger; a commit cannot embed its own SHA. Subsequent numbered
work and review fixes retain separate commits. Admitted repair integration is
`4383373272ce1d3095199caf37dde7e5d0598213`; actual history consumption precedes
final combined qualification. All preservation inputs remain unchanged.

## Implemented partial 6.3

The existing workers now each own one scalar latest-progress slot, with one
coalesced presentation wake. Identity, generation, revision, monotone byte
bounds and sealed execution lifetime are checked before replacement. The
coordinator owns export progress persistence attempts using the frozen 256 KiB
or completion threshold; callbacks never run under the queue mutex. Import
has no invented byte callback. Unsaved progress is counted separately and does
not erase the returned domain fact or independent diagnostics.

A consumer-owned projection distinguishes library generation, query revision
and job identity. Saved terminal observations outrank later running/provisional
fallbacks. Terminal results remain fetchable from the controlled report backend
after coalesced/dropped hints. This is not real SQL durability or UI qualification.

Actual app Cargo tests pass 14/14 with zero ignored/filtered: all eleven prior
controlled assertions plus bounded two-worker progress under a blocked save,
identity/monotonicity/threshold checks, and reordered saved/provisional responses.
The blocked save backend guards its terminal record rather than applying late
progress after completion. Current cumulative nextest discovery needs owner5's
additive union for the three new IDs; old eight-provider approval is historical.
One ordinary Clippy collapsible-if finding was corrected before this commit;
no warning was disabled.

Preserved NEXT6 history before this numbered commit:

- `ac25922388b533a8b7c52571979a4e4de65670b9` — partial 6.2 scheduler.
- `84af2cadea26fbcff99a225b8abb10af93ac2f67` — actual admitted repair merge.
- `a7c7f923444a6e55754072ab996bb3c41a8b2d82` — original owner5 app export and
  eleven additive actual discovery entries, consumed by fast-forward unchanged.
  Owner5 reports 63 cumulative ARM tests, two docs and full quality/fixture gates
  on that support head; this does not qualify the later progress/cancel head.

## Implemented partial 6.4

One move-only, separately measured control intent shares the coordinator's sole
call owner. Busy/closed returns the same unaccepted intent; stop remains a
separate flag and wake. A controlled recorded cancellation precedes signalling
matching generation/job workers and marking bounded queued/prefetched items.
Later source items carry the backend's recorded cancel flag. No unbounded
per-job cancellation map, caller waiter or helper task was added.

Queued cancellation skips executor invocation and still performs required
reporting. Running execution receives a cooperative work-only signal; the
returned observation and its independent diagnostics are classified as returned,
without a late-cancel rewrite. Required report/readback callbacks never receive
work cancellation. An issued uncertain control remains owned until explicit
readback even through stop or reply disconnection. Stop/control/admission are
serviced before progress/source work; progress attempts rotate worker slots and
share source opportunities when queue capacity is available.

Actual app Cargo tests pass 18/18, zero ignored/filtered, and all-target Clippy
passes with warnings denied. Four additional controlled tests prove:

- recorded-before-signal ordering under a full pipeline, one control slot,
  unchanged Busy/Closed intent, queued/future-source skipping and preservation
  of the one running executor's returned fact plus independent diagnostic;
- proven rejection and another generation do not cancel running work;
- uncertain control plus an unacknowledged report remain bounded and keep join
  pending until the controlled readback accepts the same owned allocation;
- stop returns an unissued control without claiming cancellation was recorded.

The earlier report-retention assertion still checks the two actually executed
facts/diagnostics unchanged; its five queued owners now explicitly assert the
new cancellation-before-execution observation instead of fabricated execution.
The allocation-transfer test is a symbolic owned-data witness. It proves no
real FD/guard/OS lease/custody handoff, compensation, SQL commit, postcommit
import policy, durable recovery or native presentation. Those interfaces remain
outside this released slice. Full 6.N, 6.8 and 6.9 stay open.

Partial 6.3 commit: `1a6c3db82000cb119430ee08f97ba0cea4502d4c`.
The separate partial 6.4 commit and final owner5 discovery/support SHA are
recorded externally after creation. No self-referential completion SHA is used.

## Implemented partial 6.5 — controlled terminal driver

Release `NEXT6.5-M438-CONTROLLED` extends only this private injected engine.
Each existing occupied worker can retain one move-only executor result and one
terminal driver. No extra worker, reconciler, timer thread, dependency, shared
production declaration or actual storage/custody capability is introduced.

The driver distinguishes supplied application, reported known nonapplication
and unknown acknowledgement. Readback must match the immutable generation,
job, item and complete modeled command payload. Finish payloads retain raw
nullable outcome/time values and exact counts. Unknown acknowledgement enters
readback before any further write; a matching application cannot repeat an
item count. An acknowledged CompleteItem is sticky, including when a malformed
Finish intent requires readback. A different supplied saved terminal result is
retained as actual truth/conflict, without replacing the executor fact/errors.
These response labels and public constructors are observations, not proof that
SQL, a fresh view/barrier, file custody or durable commitment exists.

Each initial CompleteItem, Finish and Interrupt command has three total writes
with virtual 10 ms spacing. After the initial Interrupt budget, reconciliation
has at most one guarded step per virtual 1000 ms, with readback after failed or
uncertain interruption. It does not restart three-write bursts. Due ticks derive
from the time observed after a callback returns; large tick jumps and blocked
calls do not produce catch-up retries. Tick regression/overflow is rejected and
deadline arithmetic is checked. There is no production clock, real-time pacing
or timer responsiveness qualification.

A failed owner derives at most one operation fence from its existing slot;
unique source exclusions are bounded by W and queued matching work stays fenced.
One latest retry cause and one latest readback cause replace earlier diagnostics.
Each retains at most 1024 UTF-8 bytes plus explicit original-length/truncation
metadata; original execution facts/errors remain intact and separately measured.
Witness and diagnostic capacity are measured separately from item/result bytes.
No per-cycle diagnostic collection or operation-to-reconciler map is retained.

A provisional interruption observation neither counts as a saved terminal
snapshot nor clears the metadata reservation or releases its owner. Stop,
cooperative cancellation and an expired observer wait cannot satisfy required
persistence/report transfer or joins. A supplied saved result can still remain
owned while actual controlled report acceptance is blocked; no already
acknowledged command is replayed during that final transfer.

Local pinned-ARM verification: all 28 app tests pass, zero ignored/filtered,
including all 18 prior identities and ten new driver assertions. All-target
Clippy passes with warnings denied. Coverage includes exact retry budgets,
continued failure then supplied saved interruption, Complete/Finish response
loss, Interrupt commit-then-Snapshot failure, wrong identity/provenance,
terminal conflicts, raw payload equality, tick jump/blocked callback timing,
checked arithmetic, 500 bounded diagnostic replacement cycles, saturated
shutdown/expired wait and actual same-allocation result acceptance. This is
controlled execution evidence, not real durable 6.5 or full-section completion.
Owner5 must pin actual additive discovery; final combined checks/native/provider
proof and an affected independent controlled proof remain separate gates.

Expanded concurrent execution exposed an existing saturation-test observation
race: a source reservation can fill the handle budget before its payload arrives.
Dedicated correction `3dcc7c5c1748bf4e38643ac024972fb20cdca2ec` waits for the
existing byte peak before sampling; every bound assertion remains. One ordinary
Clippy type-complexity finding was fixed with private callback aliases, with no
lint exception. The initial failed test and its correction are retained as
verification history rather than silently presented as an uninterrupted pass.

Historical controlled 6.2–6.4 head `f49d070dcb9d19dcb4eec17f1925e03b93591d59`
completed all eight provider gates and native review `109c4eb3` with zero required
findings. Its Intel attempt 1 failed before product build/tests when
`cargo xtask tools` could not resolve static.crates.io for powerfmt 0.2.1. Exactly
one unchanged-head retry was issued: native run `39476677836` attempt 2,
Intel job `118826483464` and aggregate `118838260408` succeeded. No source/pin/cache
workaround or current-job cancellation was used. That historical evidence does
not approve this changed 6.5 implementation; the separate commit and subsequent
owner5 support receive fresh exact-head verification. All full 6.N, 6.8 and 6.9
remain open, canonical/global-index bytes and preservation inputs unchanged.
