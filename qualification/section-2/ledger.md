# Section 2 subtask-to-SHA ledger

Owner: librairii-4. Draft [PR38](https://github.com/01max/librairii/pull/38).
Branch `rust/rewrite-02-feasibility-toolkit`, targeting only
`rewrite-in-rust-native-desktop`; no merge authorized. Initial base
`1419f1d7c5236016c0e330056c9eff45931ddf48`. Integration CI support through
`41267f6c09cb392371fdb17310914d9507a9d318` merged via `6668d0f54b8a8f03bb7e9ce36b263951db177cd7`.
Original task SHAs are retained. Initial research commits without task trailers
have dedicated task-trailed follow-ups; neither history nor evidence was rewritten.
Admitted section1 integration `9a6c1c116d9df726a061ad6aac3955df4ed41263` is now
included through merge `e28e0e66ebb69dbcea4402d462e146bb52448ff8`.
Published section3 integration `4ea2caf71bbe61ed996942daa9005de05b75f9da` is included
through merge `48fc7860faf96773047509b235586fe9b61e6de9`, preserving task history.
Ancestor `.cargo/config.toml` contains only the xtask alias; no concrete nested
target/rustflags/runner/target-dir interaction was observed or edited.
Admitted foundation wrapper repair `4383373272ce1d3095199caf37dde7e5d0598213`
is included by merge `e4e110078921841301024101e4f32048c2d97667`.
Only the integration merge brings root/shared changes; section2 makes no direct
root edits. Current combined proof is in unicode-compat/validation.md.

No external OpenSpec checkbox was changed. These statuses describe evidence,
not completed toolkit qualification. The separate local spec repository was not
staged or published. Task1 files and integration governance were not edited here;
the latter changes only arrive from the authorized integration merge.

| Task | Status | Evidence |
| --- | --- | --- |
| 2.1 | Candidate graphs/source/notices recorded; bounded Unicode comparison rejects tested final-pipeline compositions; distribution/production graph decisions pending | ADR.md, candidate-releases.json, Cargo.lock, unicode-compat/ |
| 2.2 | Incomplete: hosts/access not secured | hosts.md |
| 2.3 | Partial: compiled/running bounded synthetic ARM slice | README.md, ui/, src/; modal/responsive/complete fixture coverage open |
| 2.4 | Not observed: keyboard/AT | accessibility.md; native app access denied |
| 2.5 | Not observed: IME/DPI/editing/contrast | input-display.md |
| 2.6 | Partial: compiled picker/reveal/menu probes, no physical observations | native-adapters.md |
| 2.7 | Partial ARM normal/explicit CPU process smoke; no GPU failure | renderers.md |
| 2.8 | Partial: public component/controller route passes ARM | regression.md |
| 2.9 | Diagnostic subset only; retained budgets not all measured | performance.md |
| 2.10 | Partial:7 prior Rust +9 Python +2 child tests; original Zstd fixture;32 historical contained resource cases; optimized safety guards corrected; pre-callback OOM confirmed;24 SQL +6 filename differences retained | data-feasibility.md, corpus-observations.json, decoder-resources.md, decoder-observations.json, tests/, examples/ |
| 2.11 | Local unsigned ARM DMG only; clean-host/other packages pending | packages.md, package-macos.py |
| 2.12 | Conditional fallback not triggered; no established mandatory toolkit failure | fallback.md |
| 2.13 | Historical partial implementation approved atabdb318; new Unicode head needs fresh review/CI; mandatory qualification still pending | selection-gate.md |

Task1 PR39 is admitted at1cd2244 and merged through9a6c1c1 with146 corrected corpus
files. The2.10 consumer now pins that exact final input; its e7c771b observations
remain in prior commit history. Empty SQL prefixes, populated snapshots and
decoder capability results must not be relabelled domain or full corpus parity.
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
| Research / integration | [`e28e0e66ebb6`](https://github.com/01max/librairii/commit/e28e0e66ebb69dbcea4402d462e146bb52448ff8) | chore(qualification): merge admitted section one contract |
| 2.10 | [`d3a640c0cf79`](https://github.com/01max/librairii/commit/d3a640c0cf794e3160ca5ff443c76f67e0013250) | test(qualification): consume admitted task one corpus; verify146 file hashes, retain30 differences |
| Research / integration | [`48fc7860faf9`](https://github.com/01max/librairii/commit/48fc7860faf96773047509b235586fe9b61e6de9) | Merge published4ea2caf root-tooling integration without rewriting section2 history |
| 2.10 | [`db4153fa15b0`](https://github.com/01max/librairii/commit/db4153fa15b0b44223b63e4ebb10dfe2d98074a5) | test(qualification): add original independent Zstd producer, retained bytes/provenance and ordered payload/CRC comparisons; every prior corpus observation unchanged |
| 2.10 | [`0f7eeae7fca3`](https://github.com/01max/librairii/commit/0f7eeae7fca3f36ad4a7a349f5774438d2f5d310) | test(qualification): verify independent controls before32 decoder cases; record pre-callback resource exhaustion, OS kills/refusals, five disproved hypotheses and unselected production strategy |
| 2.1 | [`1728a13c85f9`](https://github.com/01max/librairii/commit/1728a13c85f9e9ac8721cdb4d90259ba35213001) | test(qualification): compare Unicode15 candidates with frozen Go; all1,112,064 scalars,15,550 sequences,570 retained raw differences, locked graph/notices and fail-closed protocol/supervision tests |
| 2.1 | [`d5b339faa66f`](https://github.com/01max/librairii/commit/d5b339faa66f7047faba64e59baf8458b2e6304a) | test(qualification): expose mixed-class stream-safe differences;896 inputs retain896 raw/672 stream-safe differences; original baseline unchanged |
| 2.1 | [`5cd512b31507`](https://github.com/01max/librairii/commit/5cd512b3150723474b6154e44d79e18250e0a1fc) | test(qualification): retain surviving-mark normalization counterexample;160 inputs retain160 raw/120 stream-safe and120 differences in each final route; tested final parity rejected |
| 2.1 | [`a87c1e256661`](https://github.com/01max/librairii/commit/a87c1e2566613fd7192281afb3377b720375b600) | test(qualification): guard measured normalization rejection; actual Go case0 witness must remain DIFF in all four affected metrics; six current nextest identities |
| Research / integration | [`e4e110078921`](https://github.com/01max/librairii/commit/e4e110078921841301024101e4f32048c2d97667) | chore(qualification): merge admitted corpus wrapper repair4383373, preserving every prior subtask SHA |

Later ledger/validation-only commits can be found in PR history. Exact review-ready
head is sent after pushing; it cannot be embedded as its own commit hash here.
Scoped review fixes must get new commits and be added below, never squash/rebase.

## Required review fixes

| Finding | Fix SHA | Evidence / status |
| --- | --- | --- |
| PR38 P2, comment4326316702, checkbox state after actual toggle/Apply | [`31ccd9e`](https://github.com/01max/librairii/commit/31ccd9e) | Writable two-way decision properties; actual widget Space→Apply→unchecked twice through public in-process events. Regression failed before correction and passes debug/release afterward. [Thread resolved with evidence](https://github.com/01max/librairii/pull/38#discussion_r4326422310); partial native code approval at558b2b4. New corpus code requires new final-head review; no toolkit qualification. |
| PR38 P2, comment4326769282, wrapper compiler selection/provenance | [`4a0002ac5524`](https://github.com/01max/librairii/commit/4a0002ac55243e134bd61c0e206f276be9c8468f) | Same explicit nested rustup channel and cwd for execution/Cargo/rustc provenance. Real-tool regression with older caller override passes; root invocation under unchanged stable1.97.1 records1.99.1. Admitted corpus regenerated in d3a640c; final-head rerun required. |
| PR38 P2, comment4327440644, optimized Python removed containment prerequisites | [`88e93f201a65`](https://github.com/01max/librairii/commit/88e93f201a65154abdc7089bb4dfcaa254afb04b) | Explicit runtime HostConfig/control/ELF checks. Actual controls/main regression failed before correction in both-O and PYTHONOPTIMIZE=1, then passes without reaching mocked experiments; optimized bad HostConfig never starts a child and still removes its container.9 Python tests pass normally,3 supervision tests pass optimized. Historical32-case report remains byte-identical SHA256fdaa71643e37bfea17936139765d613dd006de311d00d027d846f72cc6a6a480, tied to5d352b7 sources. Fresh final-head native review/CI required; no hostile/status-only rerun or resource qualification. |

The wrapper thread is resolved with [concrete evidence](https://github.com/01max/librairii/pull/38#discussion_r4326846323).
Historical head3131fb68109a0f82caeba1af84500ef86f20e9e5 received partial AO approval
in run ea416650 and both provider CI successes. Those outcomes remain attributed
to that head; they do not approve these new commits. Current-base local evidence
is in decoder-resources.md. Fresh native review and CI are required after push;
PR38 stays draft/unmerged. Resource, full classifier/codec/domain, physical/AT,
host, license, package and final toolkit-selection gates remain open.
