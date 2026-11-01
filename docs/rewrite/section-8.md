# Tags and independent Broken state

Task 8 starts from integration `314e7d1fdd1d0ac828e92ce77b7814cb2fa90471`
(the merge of Task 7). Its dedicated branch is `codex/rewrite-08-tags-broken`;
the task PR targets `rewrite-in-rust-native-desktop`. Production scope remains
Linux x86-64 and macOS Apple silicon/Intel; Windows support is deferred.

The implementation uses the existing schema-018 tables and owned database
workers. No migration, dependency, normal profile access, managed archive write,
or toolkit choice is introduced. Mutations use the ordinary single-writer
transaction path, preserving cancellation at admission/precommit, readiness
fencing and explicit committed/not-committed/uncertain outcomes. Callers must
interpret the outcome before offering success or Undo; raw results are data,
never mutation, recovery or file authority.

| Subtask | Implementation and evidence |
| --- | --- |
| 8.1 | Boolean/choice definitions and values, stable normalized keys, trimmed labels, accessible colors, protected edits, exact-set ordering staged in disjoint positions. Real SQLite duplicate/reorder tests; all-scalar actual frozen Go tag-normalizer comparison. |
| 8.2 | Single/bulk Boolean, replacement choice sets and additive value toggles. Deduplicated 1,000-target raw selection cap, all-target/value validation, actual change counts and no-ops. Catalog/target/mixed states share one reader snapshot. |
| 8.3 | Informed definition/value deletion plans retain an opaque snapshot of the object, counts and every referencing Collection revision/payload. Confirmation rechecks in the writer transaction. Whole Collections become `needs_attention`; criteria and approved Shelf decisions remain unchanged. |
| 8.4 | Protected Age identity and configurable precise, nonoverlapping bands. Default green is seeded only when absent; customized colors survive reseeding/reprojection. Transactional new-story and snapshot projection functions compose without nested writer admission. |
| 8.5 | Dedicated independent flag operations, availability checks, duplicate/no-op counts, cap and restart persistence. All stories remain searchable/countable and selected data is untouched. The shared export skip guard reports unavailable/Broken before file eligibility; full-scope resolution is not capped at 1,000. |
| 8.6 | Changed-only restore records, positive/distinct bounded targets and expected-current flag checks before writing. Stale/unavailable/malformed batches refuse atomically; ordinary user `broken` tags survive unchanged. |
| 8.7 | Separate read-only Standards and Spec reviews; actual Go differential captures, SQL failure injection and cancellation after real writes, strict source/notice/discovery checks. Final-head review and full local/provider results are recorded on the PR. |

## Review corrections

The all-scalar reference and independent Standards review exposed overlapping
Letter/Nd intervals: a merged table could not be binary-searched by high bound.
Separate original ordered category tables now preserve all scalars, including
Persian and NKo digits. The extraction recipe and exact leaf are provenance-pinned.
A separate red/green regression also preserves input-derived normalization bounds
for long combining-mark inputs; saved-query wire ceilings do not limit tag keys.

Spec review found that destructive reference inspection must not normalize or
execute membership. The shared strict codec now exposes bounded reference-only
decoding, matching frozen Go even when a Collection has unrelated unsupported
membership fields. Valid and already-attention Collections remain in impact
counts. Current ordinary retired-ID reuse and supported legacy retirement are
preserved. Full saved/runtime query behavior retains its original checks.

Reference captures and source hashes: `crates/librairii-storage/tests/fixtures/tags/`.
Every captured workflow field is compared without semantic normalization. The
existing frozen Go tagging/library/metadata suites plus the tagging/shared-query
and migrated-Collection/Shelf-review/Broken/export application tests passed in an
offline isolated source copy. These Go checks do not qualify missing Rust flows.

## Commit ledger

- 8.1: `265900a`; input/checkout correction `f53c103`; Unicode review fix `0d04571`.
- 8.2: `3e274b6`.
- 8.3: `8d21cbb`; reference-only review fix `0732f26`.
- 8.4: `68a5a27`.
- 8.5: `f1bd890`.
- 8.6: `8422121`.
- 8.7: the distinct verification commit carries `OpenSpec-Task: 8.7`.

Canonical task states remain in the separate local unpublished `openspec`
repository. This headless task does not implement native tag controls/feedback,
production catalog transport/activation, Collection/Shelf CRUD/review, importer,
final export preparation/copy/queued revalidation, packages or global cutover.
Those remain their own sections and PRs. An export skip result is a policy input,
not permission to copy bytes. Age startup/import/sync composition must call the
provided transactional functions when those runtimes are implemented.
