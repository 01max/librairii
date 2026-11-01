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
