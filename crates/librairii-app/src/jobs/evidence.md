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
