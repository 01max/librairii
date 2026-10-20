# Section 2 subtask-to-SHA ledger

Owner: librairii-4. Draft [PR38](https://github.com/01max/librairii/pull/38).
Branch `rust/rewrite-02-feasibility-toolkit`, targeting only
`rewrite-in-rust-native-desktop`; no merge authorized. Initial base
`1419f1d7c5236016c0e330056c9eff45931ddf48`. Integration CI support through
`41267f6c09cb392371fdb17310914d9507a9d318` merged via `6668d0f54b8a8f03bb7e9ce36b263951db177cd7`.
Original task SHAs are retained. Initial research commits without task trailers
have dedicated task-trailed follow-ups; neither history nor evidence was rewritten.

No external OpenSpec checkbox was changed. These statuses describe evidence,
not completed toolkit qualification. The separate local spec repository was not
staged or published. Task1 files and integration governance were not edited here;
the latter changes only arrive from the authorized integration merge.

| Task | Status | Evidence |
| --- | --- | --- |
| 2.1 | Candidate research/graph recorded; distribution decision pending | ADR.md, candidate-releases.json, Cargo.lock |
| 2.2 | Incomplete: hosts/access not secured | hosts.md |
| 2.3 | Partial: compiled/running bounded synthetic ARM slice | README.md, ui/, src/; modal/responsive/complete fixture coverage open |
| 2.4 | Not observed: keyboard/AT | accessibility.md; native app access denied |
| 2.5 | Not observed: IME/DPI/editing/contrast | input-display.md |
| 2.6 | Partial: compiled picker/reveal/menu probes, no physical observations | native-adapters.md |
| 2.7 | Partial ARM normal/explicit CPU process smoke; no GPU failure | renderers.md |
| 2.8 | Partial: public component/controller route passes ARM | regression.md |
| 2.9 | Diagnostic subset only; retained budgets not all measured | performance.md |
| 2.10 | Partial:7 Rust +4 comparison tests; provisional corpus retains24 SQL +6 filename differences | data-feasibility.md, corpus-observations.json, tests/, examples/ |
| 2.11 | Local unsigned ARM DMG only; clean-host/other packages pending | packages.md, package-macos.py |
| 2.12 | Conditional fallback not triggered; no established mandatory toolkit failure | fallback.md |
| 2.13 | Partial implementation approved at558b2b4; new final-head review and mandatory qualification still pending | selection-gate.md |

Task1 goldens are in PR39; its relocation-manifest correction and overall corpus
admission remain pending. The2.10 experiment pins its provisional e7c771b input
without admitting that contract. Empty SQL prefixes, populated snapshots and
decoder capability results must not be relabelled full corpus parity.
Root Cargo/toolchain/CI owner librairii-5 received pins/results; this nested spike
does not finalize that owner's graph or prevent eligible core/storage work.

## Preserved commits

| Task | SHA | Dedicated change |
| --- | --- | --- |
| Research / integration | [`6f6137412967`](https://github.com/01max/librairii/commit/6f6137412967f94c4dbef97fe0ad1534352f6c10) | docs(qualification): record task 2.1 candidate pins and terms |
| Research / integration | [`afb7e3e1ee86`](https://github.com/01max/librairii/commit/afb7e3e1ee8606100d5d1059fd8465598fd598d8) | docs(qualification): inventory task 2.2 hosts and missing access |
| 2.3 | [`9de88ea6ce91`](https://github.com/01max/librairii/commit/9de88ea6ce91e77c30b9d9d48fff8f2d34cad890) | feat(qualification): build task 2.3 synthetic Slint spike |
| 2.4 | [`ac4a3045e98e`](https://github.com/01max/librairii/commit/ac4a3045e98ed541d90cc735b19b421693f21a55) | docs(qualification): record task 2.4 accessibility access gap |
| 2.5 | [`ede80d5864ca`](https://github.com/01max/librairii/commit/ede80d5864ca9bfad1f8c5e1b6153c7d279fdc1b) | docs(qualification): preserve task 2.5 physical input gates |
| 2.6 | [`69d089debfe9`](https://github.com/01max/librairii/commit/69d089debfe9abda4c6b25dd1ce8d822082bebba) | feat(qualification): add task 2.6 confined native adapter probes |
| 2.7 | [`f442803340a5`](https://github.com/01max/librairii/commit/f442803340a502b12757cd7c0be5a84764e0383e) | docs(qualification): record task 2.7 ARM renderer smoke |
| 2.8 | [`a5d38d1c4bb8`](https://github.com/01max/librairii/commit/a5d38d1c4bb8e4b35cc114bb7795623128ecf62e) | test(qualification): exercise task 2.8 public component controls |
| 2.9 | [`1da98a5a5b31`](https://github.com/01max/librairii/commit/1da98a5a5b310a6285dbf363fe8acf22e8002fab) | perf(qualification): record task 2.9 unsigned ARM diagnostics |
| 2.10 | [`7b7415eb4601`](https://github.com/01max/librairii/commit/7b7415eb46014410451ad8604a9c509e52732ee7) | test(qualification): probe task 2.10 SQLite and archive candidates |
| 2.1 | [`99a9b798fe7e`](https://github.com/01max/librairii/commit/99a9b798fe7ed4052a573de82a6b82deed004042) | docs(qualification): identify task 2.1 resolved experimental graph |
| 2.2 | [`9b61815e2a6b`](https://github.com/01max/librairii/commit/9b61815e2a6b9d93bb9b669cc7251066812bcf5f) | docs(qualification): refine task 2.2 unsigned host availability |
| 2.11 | [`06890ee48829`](https://github.com/01max/librairii/commit/06890ee488292a8f88b30e06b5e7640946260741) | build(qualification): prove task 2.11 local unsigned ARM package |
| 2.12 | [`651010d84528`](https://github.com/01max/librairii/commit/651010d84528f6d7595c674c196fab2dcd2d6077) | docs(qualification): preserve task 2.12 fallback conditions |
| 2.13 | [`c2b23d9f5827`](https://github.com/01max/librairii/commit/c2b23d9f582779dc77dca39c2e2ed0090be3f2e0) | docs(qualification): leave task 2.13 selection pending native review |
| 2.3 | [`ea9ad3abb482`](https://github.com/01max/librairii/commit/ea9ad3abb482ec407527076d1e3a55767291f27f) | fix(qualification): remove unqualified responsive layout cycle |
| Research / integration | [`6668d0f54b8a`](https://github.com/01max/librairii/commit/6668d0f54b8a8f03bb7e9ce36b263951db177cd7) | chore(qualification): merge current integration CI support |
| 2.10 | [`954a951ab51e`](https://github.com/01max/librairii/commit/954a951ab51e38eeac38ee67de0f288a4d135e5a) | style(qualification): format SQLite extension probe |
| 2.10 | [`a3a74a3da408`](https://github.com/01max/librairii/commit/a3a74a3da408c572e9e6b3fdb91dc6c4fe53ee56) | test(qualification): compare provisional task 2.10 corpus; retain24 SQL +6 Unicode-name differences |

Later ledger/validation-only commits can be found in PR history. Exact review-ready
head is sent after pushing; it cannot be embedded as its own commit hash here.
Scoped review fixes must get new commits and be added below, never squash/rebase.

## Required review fixes

| Finding | Fix SHA | Evidence / status |
| --- | --- | --- |
| PR38 P2, comment4326316702, checkbox state after actual toggle/Apply | [`31ccd9e`](https://github.com/01max/librairii/commit/31ccd9e) | Writable two-way decision properties; actual widget Space→Apply→unchecked twice through public in-process events. Regression failed before correction and passes debug/release afterward. [Thread resolved with evidence](https://github.com/01max/librairii/pull/38#discussion_r4326422310); partial native code approval at558b2b4. New corpus code requires new final-head review; no toolkit qualification. |
