# Current performance budgets

On 2026-11-07 the owner instructed: “let bump all the 100ms to 300ms”.
This changes every current 100 ms acceptance ceiling to 300 ms:

| Gate | Previous p95 limit | Current p95 limit |
| --- | --- | --- |
| Complete 5,000-story collection query | 100 ms | 300 ms |
| Native keyboard/search dispatch under load | 100 ms | 300 ms |
| Native pointer dispatch under load | 100 ms | 300 ms |

The dispatch limit applies to both renderer routes, loaded resource runs and
the earlier native-foundation/toolkit qualification checks. Measurements,
sample counts, conservative clock bounds, callback witnesses, workloads and
cleanup requirements are unchanged. A result equal to 300 ms passes; a larger
p95 fails. These remain blocking checks on every PR and every native target.

All other limits are unchanged: literal search 120 ms, combined filters 150 ms,
six Collection counts 250 ms, deep pagination 120 ms, 24 artwork reads/decodes
50 ms, native navigation completion 3000 ms, startup 1000 ms and Shelf work
5000 ms. Runtime polling, observer deadlines and diagnostic thresholds are not
acceptance ceilings and are unchanged.

This is a prospective product-budget decision, not a performance repair or an
exception for one commit. It replaces the pending one-candidate exception
proposal for PR #86. The prior exact-source approval for `213fb525` remains a
historical decision. Fresh checks must run on the revised source and budgets.

The retained evidence motivating the decision includes PR #86's Intel pointer
p95 bounds of 89.044763–100.913134 ms and the separate diagnostic search p95
147.390984 ms (lower bound 133.751409 ms). Both exceeded their then-applicable
100 ms upper-bound gates; the causes remain unproven. The current-main query
failure of 114.806413 ms is another retained observation, not proof of a shared
cause. See [PR #86](https://github.com/01max/librairii/pull/86) for source/run links.

Historical reports, frozen Go oracle files, parity snapshots and original CI
verdicts keep the budgets under which they were recorded. They are not rescored
or rewritten as passing. In particular, the earlier 2508.940121 ms startup
failure remains above its unchanged 1000 ms limit. New passing measurements
do not establish a cause or repair for any earlier failure. This decision
authorizes neither merging nor publishing a release.
