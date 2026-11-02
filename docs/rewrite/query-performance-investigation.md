# First-page query performance investigation

The Task 19.1 Intel observation at source
`3c24629d50b7a39297c9ed05c8e317f797d7e150` failed the retained 100 ms
query budget: wall p95 was 131.736887 ms. One unchanged full-job repeat
measured 73.812746 ms. Both observations remain retained; the passing repeat
does not establish a cause or repair. P58 and the final Task 19.7 numerical
qualification remain unresolved.

The failing query plan sorts full story/archive records before returning the
first 24 stories. The existing deep-page query instead materializes the chosen
IDs and display names before loading complete records. This motivates a
controlled comparison, not a claim that payload sorting caused the failure.
Other hypotheses include summary enrichment and host CPU variability.
Process-wide CPU measurements alone cannot distinguish these causes.

## Separate CI experiment

The scale workflow runs `scale-backend --query-diagnostics` **after** the
original backend, desktop and resource workloads and their artifact upload.
It uses the same production-profile observer binary on each matching native
host, in a new process with a fresh isolated 5,000-story fixture. It never
executes against user state. No original sample, warmup, oracle, query,
assertion, ceiling or acceptance verdict is changed or replaced.

The experiment uses one database-worker transaction and compares:

- The actual first-page SQL and typed bindings from `query::explain_page`.
- The actual existing deep-page SQL, with its OFFSET changed from 24 to zero.

After one recorded warmup for each plan, twenty paired observations alternate
which plan runs first. Each execution prepares its statement, consumes all 24
rows and compares every one of the 13 SQLite column values, in order, against
the original first-page result. A mismatch fails the experiment. The report
retains both exact plans/bindings, every pair, wall and process CPU time,
SQLite VM/full-scan steps, sort counts and statement memory. These counters
describe each statement; they do not measure total process memory or disk I/O.

The separate `query-plan-experiment-<target>-<source>` artifact contains the
report, stderr and original backend manifest with source, host, fixture and
executable identities. Pre/post receipts verify the clean source and executable
SHA-256/size against that manifest. `status.json` records the diagnostic step's
actual outcome; a failure or skip cannot count as completed diagnostic evidence.
The probe has a three-minute process timeout and five-minute step timeout.
Its failure is retained separately without changing the acceptance job result.
That copied manifest describes the original backend
run, including an incomplete verdict if it failed; it does not seal or approve
the experiment. Verify the original scale archive/provider hashes and binary
binding alongside the experiment before reviewing its observations. Partial
or missing reports remain incomplete.

## Interpretation and next decision

This compares page SQL only. It excludes counting, summary enrichment, worker
queueing and the production request's complete latency. The frozen benchmark
remains the acceptance gate; this experiment grants no numerical or public
release qualification. A speed difference supports an optimization hypothesis
but cannot explain the historical transient failure by itself.

Before changing production search, inspect all three targets' complete paired
observations and exact row comparisons. Any subsequent optimization needs
behavioral regression coverage, the unchanged full benchmarks and independent
review. If the page SQL does not explain the cost, measure summary/count/worker
phases separately instead of declaring success from another passing rerun.
Human-controlled signing, final-byte qualification and release remain separate.

## Complete-request phase experiment

The reviewed three-target page-plan experiment at source
`545f37a88000816322453642abd7efccf9b51612` does not support adopting the
candidate query. Intel's candidate was slower in 18 of 20 pairs; ARM/Linux
results were mixed and order-sensitive. It did not explain the historical P58
failure. The next diagnostic, `scale-backend --query-phases`, measures the real
complete first-page search path without changing SQL, worker limits or budgets.

`query::search_observed` exposes synchronous typed phase boundaries from the
same implementation ordinary search uses with a no-op callback. Ordinary
search reads no diagnostic clocks. The observer records matching/validation,
count, page preparation, cursor stepping/record decoding, and summary enrichment.
Rows and summaries remain interleaved in their original order. Each successful
search ends with `complete`; errors never produce a completed observation.

The separate process uses a fresh isolated frozen 5,000-story fixture. One
ordinary and one instrumented warmup precede twenty pairs, alternating order.
Every full `Page` (including every summary field) must equal the ordinary warmup;
total and ordered first-page IDs also match the frozen fixture. The expected
full page is serialized, while each subsequent exact comparison is enforced by
the pinned executing assertions. This is not independent serialization of all
42 result pages.

Each instrumented observation records raw `[phase, wall_ms, process_cpu_ms]`
boundaries relative to the request, worker-entry/exit wall times, and complete
request wall/process CPU duration. Consecutive boundary differences attribute
intervals to the earlier phase; retain every row/summary interval before
aggregating. The last row interval includes cursor exhaustion and statement
cleanup. Time before worker entry includes admission, scheduling and transaction
setup; time after worker exit includes transaction completion and result delivery.
Neither is a pure queue-wait measurement. Phase instrumentation, allocations and
clock reads add overhead, so retain the ordinary/instrumented paired comparison
and order strata rather than subtracting an assumed constant overhead. CPU is
process-wide and read at different instants from wall time; CPU/wall differences
cannot establish a thread's off-CPU time or scheduler cause.

CI runs this probe after all original acceptance workloads and the existing
page-plan experiment. `scripts/run-query-diagnostic.py` provides the same
source/executable checks and timeouts for both diagnostics. Each has its own
status and archive; failure of either stays separate from acceptance. A missing,
failed or incomplete report is not a completed experiment. Validate provider
ZIP hashes, the copied backend manifest, native executable/profile and frozen
fixture hashes before admitting observations. Require 20 alternating pairs,
24 stories, complete equality assertions, monotonic finite boundaries, the
expected phase sequence, success status, database closure and fixture retirement.

A dominant measured phase guides a subsequent hypothesis test. It does not
prove the cause of the older failure, authorize a budget change, or close P58,
Task 19.7 or human-controlled release gates. Preserve the historical failure
and all unchanged acceptance results alongside the new observations.

The first phase experiment source, `f5d8a2c94fc461d47d44dd0b1ee01d3c9b3ba160`,
has independently reviewed diagnostics on all three targets. Its ARM quality
job subsequently failed the dependency gate for `RUSTSEC-2026-0433` in
`pulldown-cmark 0.13.5`, before workspace tests ran. That failed run and the
earlier successful measurements remain attributed to their original source.
The compatible `0.13.6` lockfile correction adds no waiver and does not establish
a search-performance fix. Fresh corrected-source evidence is required; the
earlier measurements do not qualify the updated dependency graph.
