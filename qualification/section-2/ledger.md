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
| 2.10 | Partial:32 historical contained cases and24 SQL +6 filename observations retained; authenticated count-guard derivative has4 safe child tests and5 new Python regressions; NEW controls +two-negative/one-benign experiment NOT RUN/pending Docker | count-budget-readiness.json, decoder-budget/README.md, decoder-resources.md, decoder-observations.json, data-feasibility.md, tests/ |
| 2.11 | Local unsigned ARM DMG only; clean-host/other packages pending | packages.md, package-macos.py |
| 2.12 | Conditional fallback not triggered; no established mandatory toolkit failure | fallback.md |
| 2.13 | Historical partial implementation approved/CI green at39267; new count-guard head needs fresh review/CI; mandatory qualification still pending | selection-gate.md |

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
| PR38 P2, comment4328610509, unauthenticated expanded-cache additions could reach oracle build | [`52fc06de1f64`](https://github.com/01max/librairii/commit/52fc06de1f640c13f0cac2b19e1e7248db5236b0) | Build module exclusively from authenticated ZIP regular members and verify exact owned bytes/inventory. Actual-main synthetic extra.go regression fails before correction normally/optimized, passes after; unverified/nonregular/unsafe ZIP refuses before source creation.15 exact Python tests pass, Unicode6 + supervisor3 also pass optimized; Rust6/6 retains actual-Go rejection. Focused Go build/provenance-only check materializes487 authenticated members; original436 source checks pass. Original measurement reports/source maps unchanged; no tampering inferred or compatibility/broad/hostile rerun. Fresh exact-head review/CI required. |

The wrapper thread is resolved with [concrete evidence](https://github.com/01max/librairii/pull/38#discussion_r4326846323).
Historical head3131fb68109a0f82caeba1af84500ef86f20e9e5 received partial AO approval
in run ea416650 and both provider CI successes. Those outcomes remain attributed
to that head; they do not approve these new commits. Current-base local evidence
is in decoder-resources.md. Fresh native review and CI are required after push;
PR38 stays draft/unmerged. Resource, full classifier/codec/domain, physical/AT,
host, license, package and final toolkit-selection gates remain open.

## QUAL2.10-392-COUNT-BUDGET — source-level partial

| Task | SHA | Dedicated change / truthful evidence |
| --- | --- | --- |
| 2.10 | [`3778a25f0b3d`](https://github.com/01max/librairii/commit/3778a25f0b3d0aaf32c774df0289e37e3fb347aa) | Authenticated79-member pinned crate;32 selected original source/metadata/notice files +one new guard module; checked pre-vector file/coder counts; typed experimental refusal; strict three-input-only Stage1 supervisor;4 safe child tests and5 new Python regressions. **No new archive constructor execution.** |
| 2.10 scoped correction | [`9887f1012a97`](https://github.com/01max/librairii/commit/9887f1012a97fbbaa70bdfff9b316ad8f9df05e5) | Zero-context derivative patch avoids whitespace-only context lines flagged by diff check. Exact33-file authenticated replay passes; derivative source bytes unchanged. Original implementation SHA retained. |

The implementation commit command continued after its patch-file whitespace check
failed; the dedicated correction above fixes that packaging issue without history
rewrite. Safe source replay, child locked build/fmt/Clippy/4 nextest tests and new
Python5 normal+5 optimized checks pass. Prior supervision3 normal+3 optimized
checks pass. Exact nextest identities are in decoder-budget/nextest-identities.json;
no ignored/filtered/skipped case. Binary-only child has no library doctest target.

New runtime controls and files-count/coders-count/solid-control executions are
**NOT RUN/pending**. The trusted LinuxARM build failed ENOSPC; after reclaiming
only owned disposable Slint output, Data had7GiB free but Docker info and daemon
ping timed out. Only own two stalled CLI clients were terminated. No shared
restart/reset/prune, unrelated-container action or uncontained fallback occurred.
Current container cleanup cannot be confirmed through the unavailable daemon.
No experimental constructor refusal, resource ceiling or production choice is
claimed. Details/source hashes and exact chronology: count-budget-readiness.json.

All historical measurement reports remain byte-identical, including original
32-case SHA256fdaa71643e37bfea17936139765d613dd006de311d00d027d846f72cc6a6a480.
The five-control/three-case runtime gate, all broader resource/cancel/codec/platform
and license/toolkit gates stay open. Final evidence-commit SHA/tree are reported
externally after publication; new exact-head native/current CI is required.

## 2026-10-23 offline Linux runtime continuation

Task 2.11 implementation `0e17aa6`, account correction `7e9f3f4`, bounded-pipe
correction `c904c06`; actual package built at `3b003cf` passes all 16 isolated
runtime checks in run40010930728. Source/image/binary/report/archive identities,
all27 runtime hashes, eight frames and independent review are retained in
`clean-linux-observations.json`. Full clean-host and macOS scope remain open.
Task 2.6 observer `00cabd1`, capture review fix `3b003cf`, and private-bus/native
selection fix `564be95` remain pending successful native integration execution.
The original failed runs remain preserved; no overall toolkit selection follows.

## 2026-10-23 verified native Linux continuation

Task 2.6 implementation `230cc47` passes all eight cases in each X11/Wayland
session in run40018196257. Independent review approves all16 cases,190 session
file hashes, exact portal responses, raw X11 parent properties, six Wayland
parent associations through teardown, both actual Thunar paths and all711
matching archive payload hashes. Picker images precede repaint; native row
observations plus exact callback results prove selection. See
`native-linux-observations.json` for immutable source/binary/artifact identities.
RFD cancellation and blocking cold folder reveal are fixed in separate commits.

The same run revalidates all16 offline runtime checks against the matching package.
Independent review verifies all27 runtime hashes and eight frames, identical to
the earlier visually reviewed French/CJK images. Task 2.11 test-only correction
`229c68f` makes child readiness deterministic before the unchanged deadline
assertion. All61 Python tests pass; run40019286663 passes Linux and both native
macOS lanes, with Rust quality40019286755 and existing CI40019286676 also green.
These outcomes belong to those recorded heads; subsequent evidence commits are
verified separately. Physical/IME/Orca, remaining macOS integration, complete
clean-host/package, license and toolkit-selection gates remain open.

## 2026-10-24 disposable Git fixture cleanup

Existing CI at `5de32ce`, run40081026303/job120834442189, retains a failure in
`test_legacy_and_unknown_paths_run`: all decision assertions execute, then
TemporaryDirectory cleanup reports `Directory not empty: '.git'` on hosted
Git2.55.0. A background repository writer is consistent with that failure;
the log does not identify the writer. The disposable test repository now sets
local `maintenance.auto=false` and `gc.auto=0` before its first commit, preventing
automatic maintenance from outliving a fixture command. Cleanup errors remain
fatal, all seven decision tests and their assertions are preserved, and neither
the production decision nor any real repository's configuration is changed.
All seven tests pass locally; fresh hosted execution must verify the correction.

## 2026-10-24 native recorder exit race (2.8)

At `6da2838`, run40093774916/job120871675095, Intel's recorder suite fails
before native interaction: the excess-output child exits between its observed
running state and group cleanup, and Darwin `killpg` returns EPERM. A local
owned-child reproduction observes state Z, the same EPERM, exit0 on reap and
ESRCH for the retired group afterward. The deterministic regression reproduces
that failure before the correction and passes afterward. Cleanup now reaps an
actually exited owned child on EPERM and still requires the subsequent group
signal to succeed or report ESRCH. Live permission denial remains fatal; output
limits, deadlines and descendant checks are unchanged. All nine focused tests
pass locally. This correction is not native interaction evidence.

Apple's [XNU group-signalling implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c)
filters zombie members before deciding EPERM/ESRCH. The retained failed run is
not retried or relabelled as passing.


## 2026-10-24 configured Orca baseline (2.4, partial)

Implementation `0c1b202b5a677f91e50136b9e513b8e24c48b7e7` passes all three
native qualification lanes in run40097549724, Rust quality40097549732 and
existing CI40097549800. Linux job120882860176 records both configured-v3
Orca sessions passing all ten baseline checks and all six completed speech
stages. Artifact12562215393 retains the original report,51 session hashes,
startup-excluded PCM/WAV, native focus/text snapshots and effective preferences.
See `accessibility-linux-observations.json` / `configured_v3` for exact identities.

X11 action-key echo is disabled only in this private reader profile; its earlier
default-echo failure is retained. Wayland retains action-key echo and the app
has no DISPLAY; the reader uses an isolated XKB compatibility display. Secondary
unprompted offline base.en transcription recognizes main labels with some
misheard words. No human hearing, complete intelligibility, virtualized review,
progress/error, full pointer-free workflow or toolkit qualification is inferred.

Independent scoped review approves the original archive/package, all51 session
hashes, all12 exact utterance/PCM/quiet-tail stages, startup exclusion, native
focus/text, private preferences, Wayland protocol and owned cleanup. Both filtered
images show `0042`, one row and French/CJK. The supplementary ASR transcripts
and pinned model bytes are verified; the explicit scope limits are preserved.


## 2026-10-24 verified execution hosts (2.2)

Independent Task2.13 scope review confirms that2.2 can close with the verified
native execution routes, working SDK/CI prerequisites and proposed baselines.
Existing `macos-15`, `macos-15-intel` and `ubuntu-24.04` runners repeatedly execute
matching packages and actual OS controls; Intel uses a real i7-8700B without
Rosetta. All three lanes pass at0c1b202/run40097549724. Updated `hosts.md` preserves
historical access gaps while identifying current reviewed routes and limits.
The lack of owner-operated Intel/Linux machines is not an additional host gate.
Remaining accessibility/input/display/performance/clean-package/selection work
stays open. No new host, paid runner, account or system permission is provisioned.


## 2026-10-24 native macOS composition method (2.5)

The disposable-CI-only input observer uses public TIS/CGEvent calls and Slint's
pinned versioned public Winit hook, always propagating actual IME events.
It requires exact French dead-key/AZERTY output and Japanese preedit,
candidate/commit/cancel, then restores original input-source selection and all
enable states even on failure. The method preserves normal shutdown and bounds.
First actual ARM/Intel native execution and independent evidence review remain
pending; Linux composition and display/contrast/scaling gates stay open.


The versioned Winit feature adds one direct Slint→i-slint-backend-winit lockfile
edge, without changing a package version/checksum. The follow-up refreshes the
recorded GUI lock digest and resolved Slint dependency list to that actual lock.


Independent review P2 correction: the IME cancellation check previously accepted
an empty preedit emitted before the live composition. It now takes a fresh trace
offset immediately before Escape and requires the final preedit after that
offset to be empty, with no commit anywhere in the cancellation phase. The
negative early-reset→live trace reproduced false acceptance before the fix;
all four focused tests pass afterward. The raw full and post-Escape traces are
retained separately. First actual native execution is still pending.


Independent review P2 setup correction: read-only UCKeyTranslate on Apple's
French layout proves physical0 maps to q with plain/Command modifiers, while12
maps to a. The new composition probe therefore uses the actual native Edit→
Select All menu followed by physical Delete to clear Search, avoiding the
existing ABC-specific Cmd-A keycode under French or its underlying IME layout.
The required physical French/Japanese input and exact text assertions remain.
No local source selection or keyboard event was used to establish that mapping.


Task2.5 first native ARM execution at3cef8c1/run40100102140/job120890317005
fails before any source mutation or text input: the absent legacy Kotoeri bundle
filter returns nil and the helper force-unwraps it. The package's existing eight
native cases pass; the failed input app still exits0 and removes its fixture.
Artifact12562189090 ZIP SHA256a9d14f40ec6490d12f504f4e5147af46a2e57b23364f4e5b93f38f1455968ef6
retains the exact failure. A local read-only TIS reproduction finds six current
RomanTyping sources, nil for the legacy bundle, and the French layout; selected
ABC is unchanged. The observer now treats nil as an empty inventory; Japanese
availability/selection and all input requirements remain mandatory. A native-only
regression compiles the real TIS helpers, verifies a missing family is empty and
French exists, and confirms original selection is unchanged, without AX/actions.


Task2.5 native editing diagnosis:0c6c073/run40100937198 passes the full Linux
lane; both Mac input runs fail after native Select All/Delete. ARM raw evidence
proves French dead-key/AZERTY exactêéaz and Preedit(^)/Commit(ê), then only the
last z is deleted. Full source restoration and owned normal shutdown pass.
Methodv2 adds bounded input-only native command/guard/focus/key observations and
read-only AX selected-text/UTF-16-range readiness before Delete. One Select All
command must establish full real selection; no forced text/focus/selection,
command retry, relaxed deadline or inferred Japanese pass is introduced.
See input-display.md for immutable failed artifacts and method distinction.


Independent review P2 correction to methodv2: input-selection previously echoed
the requested PID alongside an actual focused element. It now requires a
successful AXUIElementGetPid on that element, exact requested-PID equality, and
returns the observed element owner. A foreign IME/accessibility element cannot
supply the owned-field witness; the immediate native foreground check remains.

Task2.5 native runf462e38 retains two different input failures: ARM delivers the
menu command and passes the actual widget guard but aborts on the first empty AX
focus response; Intel cannot find Select All. Full Linux and quality CI pass.
The reader correction records the native status and bounded owned activation
tree as an incomplete observation, permits read-only bounded polling for lazy
AccessKit initialization, and requires actual AXFocused=true before accepting
the full text/UTF-16 range. No action retry or selection/focus mutation is added.
The focused seven-test suite and Swift compilation pass; fresh execution remains
required. Immutable artifact identities and raw-hash verification are recorded
in input-display.md; earlier failed verdicts remain unchanged.

Task2.5 separate menu-reader correction: the composition driver names the native
Edit submenu, and the helper opens that exact unique submenu once before
inspecting its children. This addresses lazy AppKit menu accessibility lookup;
it never retries Select All. Item metadata and the open-before-lookup fact are
retained, while absent/ambiguous submenus/items still fail. Existing adapter
menu lookups preserve their behavior. Native execution remains required.

Independent review correction: the new explicit-submenu route now finishes its
bounded read-only item traversal and requires one exact matching item before
the action. Missing/duplicate items or incomplete traversal fail before the
single item press, making the documented ambiguity guarantee concrete. The
legacy no-submenu route retains its original first-match behavior.

## 2026-10-24 conservative native notice supplement (2.13)

Cargo's Rust notices plus one Skia BSD text omitted the native source-graph
supplement. The new offline inventory/helper packages pinned original notices
and the IJG binary acknowledgment for ten native components, including separate
skcms, Chromium/ARM/Intel, VMA and Khronos terms. It verifies the actual binding
version/Skia ref, native build BSD and complete notice bytes before copying;
macOS resources are sealed afterward. Four focused tests and a real offline
Cargo/native-output verification pass. Archive member presence is not treated
as final-executable linkage, and final per-target distribution review remains
open. See native-notices.md for scope and source inventory.

Independent review P2 correction: the Chromium zlib attribution list omitted
the2018 cpu_features.c and2019 insert_string.h copyrights. Exact pinned primary
headers (Git blobs64e0428cd2fc2d965cf18c2af9212df68d2ea8c8 and
260b8268029a8c0bc03055ba2ac2dcb19c277c9d) are preserved verbatim in a separate
source-notice file, both source paths/blob identities are inventoried, and all
affected payload hashes are refreshed. No source-graph or license change occurs.

## 2026-10-24 native input and desktop readiness follow-up

The5f1606d ARM/Intel runs both prove exact French full AX selection and clearing.
ARM emits Japanese preedit endingにほn; the separate09f6f76 correction types the
additional final physical n. Intel instead commits Latin input after a Disabled
transition. The v3 observer records an optional two-second public application
launch-notification observation, without adding a capability prerequisite;
actual IME assertions remain decisive. Independent review corrected the initial
interpretation of finishedLaunching before publication. All100 recorder tests
pass after removing only a generated Swift cache which caused a confirmed local
ENOSPC compile failure; Swift compilation also passes. Fresh native CI remains
required. Separately reviewed desktop setup corrections are da73c0e (real X11
handshake/live Openbox registration) and ce35c7c (incomplete busy Finder reads).
See linux-vm.md, native-macos.md and input-display.md for preserved failures.

## 2026-10-24 owner language qualification scope (2.5)

The owner prioritizes UTF-8/French-Latin and defers Japanese/Chinese/Arabic
input/IME/shaping qualification. The default Mac v4 gate retains actual French
dead-key/AZERTY input, explicitly required native full selection/clearing,
restoration and normal retirement; Japanese setup/input runs only when the
deferred diagnostic is explicitly requested. Tests distinguish the default
scope from an explicitly requested diagnostic and reject missing witnesses.
All101 recorder tests pass. The uncommitted native reference was withdrawn.
Earlier failed runs retain their methods/verdicts; UTF-8 data handling and
existing valid-string fixtures are unchanged. Fresh scoped native CI is required.

The same2.5 continuation adds explicit Linux X11/Wayland dead-key/accent input
and native UTF-8 clipboard round-trip checks to the early-Settings case. One
native keysym sequence must produce `êé az`, then Copy/Cut/Paste must restore the
actual value; no Unicode text payload supplies composition. New required checks
reject incomplete session summaries; all eight focused Linux recorder tests pass.
The private desktop/VM routes and existing deadlines/retirement remain unchanged.

Independent review found that immediate Copy/Cut could hide a broken Copy by
overwriting its clipboard result. The separate correction observes Copy,
physical deletion and exact Paste before the independent Cut/Paste sequence.
Distinct actual states and an additional screenshot retain both witnesses;
a regression rejects no-op Copy, Cut and Paste rather than trusting the final
string alone. No app value setter or clipboard setter supplies these results.
Command review also distinguishes XTest's temporary private scratch mappings
from Wayland's single uploaded keymap; exact observed composition remains required.
Independent scoped re-review approves the correction and all15 focused Linux/VM
tests. The updated full102-test recorder suite passes locally, including the
VM fixture's new input witnesses. Fresh native v4/v2 execution remains pending.
