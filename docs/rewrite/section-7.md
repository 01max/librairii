# Search, query codecs and navigation

Task 7 is delivered against `rewrite-in-rust-native-desktop`, starting at
`b90d30e99218ae2687bbd84f853e3e39af10505c`. Required production targets are
Linux x86-64 and macOS Apple silicon/Intel. Windows support is deferred.

Each subtask has a distinct Conventional Commit with an `OpenSpec-Task` trailer.
The canonical OpenSpec checklist lives in the separate local `openspec` repository.
This document records shipped scope and verification; pending rows are not passes.

| Subtask | Scope | Evidence |
| --- | --- | --- |
| 7.1 | Frozen Unicode runtime normalization | Original `191ae6c320778467b71a3d0d25a3efde61e30ebc` retained through a normal merge; one shared frozen pipeline, distinct saved/runtime passes. ARM Rust 1.99.1: core nextest 132/132, zero skips, UUID `eeab244d-7362-466b-ac86-acc3ce8c8f3a`; includes complete scalar/sequence references and 14 runtime identities. Source-policy 22/22; core all-target Clippy with warnings denied passed. |
| 7.2 | SQL search/count/page, filters, ordering and indexes | ARM: 5/5 isolated SQLite regressions, zero skips; UUID `610bcdc6-5728-42b5-937c-3f4bacbf3f38`. Live ignored/kind/value validation; literal wildcard/Unicode; active metadata/language independence; sort ties and huge pages; actual predicate EXPLAIN uses retained indexes. Storage Clippy and source gate passed. |
| 7.3 | Bounded saved membership and live-reference validation | Reuses frozen v1–v5 decode/migrate; adds current canonical encoding and same-transaction validation. ARM query suite 7/7, zero skips; UUID `bef9d2df-fcd7-4390-bcd4-a1bdaad6e2d2`. Nested invalid references refuse whole queries; retired IDs/current ordinary reuse and node limits covered. Core/storage all-target Clippy passed after correcting a local const-reference warning. |
| 7.4 | Current/legacy URL and typed scope decoding | ARM URL suite 4/4, zero skips; UUID `7816ea58-1529-4ebe-b974-8993cbe6964d`. All nine shared fixtures, nested retired references, current tag reuse, malformed/versioned/duplicate input, node/byte bounds and matching-hash Collection/Shelf identity are covered. Core all-target Clippy passed. |
| 7.5 | Semantic navigation and native preference persistence | ARM: 6/6, zero skips; UUID `def35ea3-27d6-46f7-95bd-bed773680f2e`. Back/forward/branching, typed Shelf/manual order, selection/refinement/page/sort, independent presentation, raw text/one-pass reopen, stale stamps, profile restart, unknown versions and symlink preservation pass. Core/storage all-target Clippy passed. |
| 7.6 | Full-scope snapshot resolution and differential references | Pending |
| 7.7 | Independent review and edge/stale-response verification | Pending |

The 7.1 merge preserves all 329 integration-base Rust discovery objects and all
38 Python identities, appending 14 Rust and four Python identities. Both source
bundles, original codec delivery lineage, frozen references, notices and strict
selected-closure checks remain mandatory. Conflicts were resolved by retaining
both provenance contracts and recording additions before historical comparisons.

Parity coverage: normalization/search P14 and query/navigation P15–P17;
exact row mapping and final cumulative/provider results are recorded as each
implementation boundary is verified. This headless work does not qualify a GUI,
toolkit, package, production import, metadata synchronization or Windows writes.

Native navigation keeps at most 100 history frames and an 8 MiB serialized
preference document; a request retains the existing 262,144-byte wire bound.
Invalid/oversized changes refuse atomically rather than truncating selection or
query predicates. UI preferences live in the separately leased native profile,
not the movable library. Publication reuses the anchored create/EXCHANGE/flush
protocol, retaining prior/new readable witnesses and fencing uncertain results.
Unknown/future preferences are preserved; no preference file grants activation,
recovery or mutation authority. Selection/presentation changes retain the query
stamp; semantic navigation invalidates it, including Back to an earlier view.
