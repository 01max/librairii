# Rust-only runtime and historical retirement plan

**Current scope (2026-11-03):** the owner approved a fresh Rust app and retirement
of the old runtime without Go data migration or paid Mac signing. The
[fresh Rust contract](fresh-rust-contract.md) supersedes the original bridge,
signed-release and legacy cutover prerequisites below. The original inventory
and observations remain historical evidence; the Task 20 implementation records
its actual removals separately.

## Cancellation backpressure correction

Source `9ccfd31b701147ce3152c78e80d102779bf39299` passed the Linux deep
suite (642 tests), ARM/Linux quality and all three installed-package workflows.
Intel quality ran 640 tests: 639 passed and the held-import cancellation test
failed on `storage.queue_full`. Its shared assertion did not record the action,
so this observation does not identify an import, cancellation or database-read
cause. The original failed artifact
[13302145686](https://github.com/01max/librairii/actions/runs/41784731927/artifacts/13302145686)
remains retained with SHA-256
`94b14902d02f807ede49e5d188980f9078e967fb983084a331fa602a5c9956f7`.

The test previously required every offer to succeed immediately, contrary to
bounded scheduler backpressure. It now keeps action/revision/state diagnostics,
retries read-only observations within its original eight-second deadline, and
repeats imports only with an explicit non-admission receipt and no new operation.
Cancellation now exposes a distinct pre-acceptance Busy receipt; closed intake
and a failed read after accepted cancellation cannot authorize another offer.
Both the test and native component fixture require this exact-revision receipt
before repeating cancellation of the same observed active operation.

The test observes each saved `cancel_requested` flag before cancelling the next
job. A published presentation revision alone does not prove that the scheduler
consumed its single cancellation slot. Waiting for terminal completion at that
point could instead block a queued batch behind the other deliberately held
batch. Both imports must still remain active while real copy progress appears,
then finish with all 64 items cancelled, preserve source bytes, join workers,
and reopen both complete paged reports. The correction changes no performance
budget, worker capacity or automatic application retry policy. Fresh native CI
must validate it; the historical failure is not relabeled as passed.

## Current implementation

The [actual retirement record](legacy-retirement.json) binds 377 removed tracked
runtime/tool files to their pre-removal hashes and preserves all 49 shared inputs.
Before removal, 251 additional original module files were retained under
`preservation/oracle-files/`, verified against the unchanged original oracle
manifest. The optional reference generator now refuses missing archived files;
it never falls back to the working application tree.

Production startup offers a fresh Rust library and ignores old Go settings,
databases and journals. The desktop preference-transfer control is removed.
Native preferences, library reopening, relocation and recovery remain. Both
installed renderer routes start fresh; the software route includes old files
and verifies their bytes are untouched. All 22 installed workflow checkpoints
and PR-blocking performance budgets remain required.

Make commands, dependency automation and product CI now target Rust. The old
Go/Wails jobs and bridge rehearsals are retired; manual native candidate reruns,
three-target quality/scale/packages and focused Rust checks remain. The Mac
builder provides free ad-hoc DMGs with first-launch instructions. Protected paid
signing remains optional, human-controlled and separate from normal CI.

A fresh detached checkout of `171949e705351137f27b0c4ac03f7839e0d331f2`
passed all **135 cumulative headless tests**, exact source/corpus/migration-byte
checks, formatting and the no-side-effect Mac build preview. The local receipt
is `target/task20-fresh-rust/clean-headless.json`; the combined log SHA-256 is
`65db658f9cc030c4a565046f21fefa0e857e26f1dc3aa24eb7d82f356d43b49a`.
The original checkout also passed all Python parses, inventory verification and
frozen-module materialization. These are headless checks, not native app execution.
After the storage recovery corrections, both independent Standards and Spec
source reviews reported zero required findings at
`2737e3c836a7838321fd1d1b3fefb9ba8747647d`. The same 135 headless tests and
source/corpus/migration checks passed again. Review covered missing Rust settings,
language saving after failed recovery, old namespace files and dangling links,
earlier native v2 records, and current aliases into native data.

[Task 20 PR #76](https://github.com/01max/librairii/pull/76) records the selected
CI source, current three-target results and independent evidence review. Native
compilation, regression tests, both fresh installed renderer routes and the
unchanged performance budgets must pass on that source. Source review and
headless checks do not substitute for those results. Preserve failed runs with
their original verdicts when recording a corrected run.

The first Rust-only CI source, `db624e2c6e64ab62ff873265bfa8b4142f55706e`,
built all three packages. ARM/Linux scale and Intel installed workflows passed.
Workspace/deep checks exposed an unused copy-binding route, incomplete test
close lifecycles and a missing explicit frozen-copy source-journal guard.
Installed ARM/Linux checks also caught the observer querying SQLite between
profile publication and the live-library handoff: even its read-only connection
left WAL/SHM files. Separate commits correct these paths; a real SQLite workflow
replay fails before the observer fix and passes afterward in both languages.
The optional signing helper now consumes the actual installation text rather
than a stale format. Both free Mac instruction documents remain byte-identical.
The corrected headless suite contains **136 passing tests**; these checks do not
exercise native apps or Apple signing services.

That first source's Intel resource warmup failed startup at **2740.975 ms**
against **1000 ms**. Idle CPU (**2.358%**) and peak RSS (**127,389,696 bytes**)
passed. Both independent evidence reviews verified the archive, source and
1,193 resource seals; no measurement defect or causal fix was established.
Only one of eight resource pairs completed, so its four passing display runs
do not establish resource qualification. The original failed verdict remains.

The corrected source `8c8fea5ae283e762010b2bf01c3166269c408968` passes
all 639 deep regression tests, but its Intel resource warmup also fails startup:
**1687.514 ms**, with idle CPU **2.202%** and peak RSS **127,266,816 bytes**
within budget. Its first snapshot arrived at 8.362 ms, window construction at
1081.591 ms and first UI timer at 1631.781 ms on the child's clock. This again
localizes most delay before the constructed-window boundary; it does not identify
the responsible operation. The original measurements and all 1,193 resource
seals remain retained. Diagnostic source `b81f37dc284edc406e7eeec232f9e5eeaf48055a`
adds temporary `[DEBUG-task20-startup]` timings around toolkit, component and
menu initialization. Those timings are removed from the final implementation.
The probe changes neither execution order nor acceptance thresholds and is not
a performance correction; its results do not substitute for uninstrumented
final-source qualification.

That diagnostic source completed all eight Intel resource pairs and four display
runs. Startup ranged **309.593–535.527 ms**, maximum idle CPU was **3.197%**,
and maximum peak RSS was **134,942,720 bytes**. All 1,375 resource seals verify;
all product workers joined and their fixtures retired. The tagged toolkit stage
ranged 157.684–306.273 ms. This passing run did not reproduce the earlier delay,
so it identifies no cause or performance fix and does not qualify later source.
Raw [artifact 13284361798](https://github.com/01max/librairii/actions/runs/41741567592/artifacts/13284361798)
is retained with SHA-256
`9d6527d33af9eb8e697a9b05251602f50736b2448379f36ac19babf195dd4b1e`.

The diagnostic source `b81f37dc284edc406e7eeec232f9e5eeaf48055a` also
exposed a timing dependency in the ARM native cancellation fixture: both batches
of 32 tiny duplicate archives finished before the required Cancel callback. Its
retained snapshot shows successful operations and `storage.snapshot_changed`,
so this run does not establish a failed cancellation of active work. The fixture
now supplies the existing import effect observer through the native controller,
holds actual read/copy boundaries, and releases one chunk for the original
progress/validation-notice assertion. Real cancellation releases the held work;
cancelled-item reports, source preservation and reopen remain asserted. Ordinary
startup supplies no observer, and no environment variable enables it. A new
cumulative controller regression exercises two held imports, real progress and
paging, cancellation and durable reopen. Native execution of this correction
still requires fresh CI; headless checks cannot certify it.

Source `842d3939e6cd5e6bb29c52d57c5c2d89454aa97a` reached that new
controller regression and exposed its incorrect reopen assertion: compact notices
show only the latest terminal import, while History retains both cancelled jobs.
The correction explicitly opens History, checks both exact job identities and
cancelled totals, then reads both pages of each report and verifies 32 unique cancelled item
identities per job. Commands wait for their matching published revision.
The original active-work, real-progress and source-preservation checks remain.
The failed run remains retained; execution of the corrected test requires fresh CI.

The same diagnostic source failed the Intel installed interruption prerequisite.
The native Sync click was performed, then the app displayed `storage.queue_full`;
all seven retained jobs were terminal and no new request reached the local catalog.
The archive preserves that failure. These observations do not distinguish the
scheduler's momentary lock contention, readiness and occupied admission slot.
A generic error can also follow an accepted offer, so it cannot authorize replay.

The correction preserves the scheduler rules and adds an explicit user retry
only for the exact owned metadata command returned unsubmitted with Busy.
The receipt binds its identity, command revision and library generation. Retry
consumes that same command; closed/fenced/stale/consumed receipts cannot submit
it again. The installed observer permits one click on this distinct retry control
within the original ten-second request deadline. Missing SQL/HTTP or a generic
error never causes a repeated Sync click. A real paused request, one durable
running job, exact SIGKILL and retired capture remain required. Early failures
now also release the fixture and retain an interruption receipt. Readiness, the
actual native action boundary and the event witness share the original deadline;
late success fails, with headless clock replays covering each boundary. The headless
replay fails before this observer correction and passes afterward in English and
French, including persistent refusal and uncertain-result cases. All 138
headless tests pass; the Rust regression and native path await fresh CI.

After both corrections and removal of the diagnostic logging, a clean detached
checkout of `297ab21b87f5befd1a796d55cbbb3c873e5ef2fc` passed all **138
headless tests**, exact source/corpus/migration-byte checks, formatting and the
non-executing Mac build preview. Its log SHA-256 is
`330d90183bb4c66f32240f432bef1416f70ff7f510b6752bdb5dede73562aa82`.
The changed Swift observer also passed local type-checking without execution;
its dedicated compiler cache was removed afterward. Fresh Rust/native CI remains
required for the corrected source.

The next source, `d3e57b28bf5ece60af458bc5dfdd7711e2b0a13a`, failed
before Rust test execution: the maintenance example omitted three newly required
optional observer fields, and the sync retry module used `expect`/`unwrap` in
violation of the existing lint rules. The correction supplies `None` for ordinary
maintenance fixtures, returns a checked error for an absent retry input, and
propagates test setup errors without changing the assertions or lint policy.
All desktop targets and examples then passed local compile-only Clippy with
warnings denied; all 138 headless tests and source/corpus/migration checks pass.
Both independent source reviews report zero required findings. Original failed
logs and provider-verified archives remain retained; corrected Rust tests and
native execution still require fresh CI after the current jobs finish.

That source's Linux installed X11 GPU lane failed its initial foreground
prerequisite, before any workflow check. The same-PID accessibility application
had registered, and the product reported Setup at 69.661 ms and continued drawing
frames. No retained active-window observation establishes why foreground
ownership was absent. The process was killed and reaped with its reader retired;
that cleanup is not a graceful worker shutdown. The other three Linux installed
lanes and both Mac installed checks passed, without overriding the failed lane.

The Linux observer now requests native activation once for one visible window
whose actual `_NET_WM_PID` matches the installed process, then requires the exact
window to become foreground and rechecks its owner. Window discovery, activation
and observation share the original 15-second foreground deadline. Before/after
properties and refusals are retained. Ambiguous, missing, foreign, late or
ineffective activation fails without replay. Controlled headless regressions
establish this observer contract, not the historical cause; fresh installed CI
must validate the actual native route. The failed archive remains retained as
[artifact 13286289297](https://github.com/01max/librairii/actions/runs/41748877717/artifacts/13286289297),
SHA-256 `9680c882b7a433c826efda8209682bc0e78961b1120dff3a1a5ffe75760dfe19`.

The same source's Intel software resource warmup failed actual keyboard dispatch:
its conservative p95 interval is **100.276104–113.836529 ms**, with observation
wall p95 157.002305 ms. Four samples have lower bounds above 100 ms; their native
posting calls returned in 0.091443–0.394185 ms. This locates the excess after
posting and before callback return, without identifying event delivery,
main-thread scheduling or handler work. Pointer 50.091044 ms and completion
1319.232140 ms passed. Seven input cohorts and their joined event logs verify;
three later software resource pairs are absent. Its failed
[artifact 13286814027](https://github.com/01max/librairii/actions/runs/41748877688/artifacts/13286814027)
has SHA-256 `c11ccb36a1b9a0d67479b1b6840a6b6a32aaccd11a23fb9bfeddd243c7754ec2`.

A temporary qualification-only main-thread probe recorded timer stage timestamps
on the same clock as the dispatch witnesses. It retains bounded in-memory records
for slow timer work, long gaps between timers and changed search sequences,
writing them only after worker join. The probe changed neither workload nor
acceptance limits. Its code and probe-only source pins are removed after retaining
the raw observations. Fresh uninstrumented source qualification remains required;
removing diagnostics does not establish a performance correction.

Diagnostic source `842d3939e6cd5e6bb29c52d57c5c2d89454aa97a` completes
all ten Linux input cohorts: 640 keyboard samples, 640 pointer samples, 200 cold
completions and 1,910 matching joined events. Both independent reviews verify
the 27 frozen inputs and all 8 backend/37 desktop/1,383 resource seals. Maximum
cohort p95 is 20.952015 ms for keyboard, 14.148170 ms for pointer and 408.940778 ms
for completion. All eight resource pairs and display checks pass; maximum startup
is 529.234253 ms, idle CPU 0.815399% and RSS 235,532,288 bytes. The ten diagnostic
traces have no overflow and no slow callback samples. This Xeon host differs
from the earlier EPYC failure host. The passing observation establishes neither
a historical cause nor an optimization and cannot qualify later uninstrumented
source. Raw [artifact 13294200973](https://github.com/01max/librairii/actions/runs/41766355629/artifacts/13294200973)
is retained with SHA-256
`3f09f7faf510a7c420760cb071d8a56344b8e2f858299ab48cb184d708917eaf`.

The same source's ten Intel input cohorts pass their existing p95 gates, while
two individual callbacks are certainly above 100 ms: keyboard sequence 1 is
bounded at 306.372212–319.633867 ms and software pointer sequence 5 at
146.842726–158.747699 ms. A third pointer interval spans the threshold
(88.515100–100.420073 ms). The first two guaranteed dispatch intervals fall
entirely between recorded timers, within gaps of 897.853555 and 184.311300 ms.
Posting takes less than 0.4 ms. These observations do not attribute the delay
to the recorded timer bodies or distinguish rendering, other callbacks, event
delivery and host scheduling. No cause or optimization is established, and
the historical failed resource cohort keeps its verdict. Raw
[artifact 13294806397](https://github.com/01max/librairii/actions/runs/41766355629/artifacts/13294806397)
is retained with SHA-256
`20ea47568e4f89a0abf9855d9c01d2039642b0e2c9c4d6c69a6a0dff395d9ee6`.

### Linux dispatch measurement

The same `d3e57b2` source failed its first Linux GPU resource warmup at
**126.699703 ms keyboard observation-wall p95** against 100 ms. Pointer
26.648978 ms and cold completion 382.583897 ms passed. The seven later resource
pairs are absent. The original failed [artifact 13285297138](https://github.com/01max/librairii/actions/runs/41748877688/artifacts/13285297138)
is retained: 131,368,054 bytes, SHA-256
`73ca978a1da37319565552bce4245a7a07175241ee152bf830fc142ce681164e`.
Both independent reviewers verified the source, all 8 backend/35 desktop/1,208
resource seals, 64 keyboard and pointer samples, all 64 clock rounds and the
joined 191-event native log, with positive worker join and fixture retirement.

Those clocks conservatively bound keyboard callback p95 at **9.953264 ms**.
Of ten keyboard observation walls above 100 ms, nine retain a minimum of
105.852085–137.126096 ms after the actual callback. This locates the excess
interval after dispatch; it does not distinguish timer/projection/render work,
file publication or observer scheduling, or establish a product optimization.
The raw run remains failed and does not qualify missing pairs or later source.

Future Linux runs use the existing macOS method3 protocol: time from before the
native input helper starts to the actual production search callback return or
browser-focus acknowledgement, using the conservative bound from all 64 clock
rounds and exact state/sequence correspondence. All original observation walls
remain in both the raw file and report. Both platforms require the complete
event log to match after positive worker join before marking a report complete.
No new per-event arming, waiting or preconditioning is added. The 100 ms dispatch
and 3000 ms cold-completion gates, samples, imports, covers and renderers are
unchanged. Callback bounds do not claim visible completion.

Headless regressions exercise the real Linux admission and inspection paths
with inert processes: late observation remains visible, actual callback and
event-posting delays still exceed the dispatch gate, missing clocks refuse
admission, and a mismatched joined event refuses completion. A provenance-bound
64-sample Linux trace preserves the original failed wall separately from its
callback bounds. Fresh complete three-target CI remains required.

Remaining release work is selection and independent qualification of exact
Rust-only distribution bytes, reconciliation of the recorded P58 performance
hold, and an explicit publication decision. Go bridge/upgrade rehearsal and paid
Apple signing are outside the required path. Optional paid-channel scripts stay
human-operated. The local OpenSpec handover tracks task completion separately
from release promotion and archival.

No historical performance failure is rescored, no public release is approved
and no personal data is removed by this change.

## Historical Task 20.2 plan (superseded prerequisites)

The remainder preserves the original plan and observations. Its references to
still-present Go code, compatible bridge/rollback and mandatory paid signing
are historical; the current contract and retirement record above govern this PR.

Task 20.2 prepares a removal manifest. It does **not** retire any runtime,
select a release candidate, approve cutover or complete Tasks 20.1/20.3–20.5.
Signing/final-byte qualification and the historical P58 performance hold remain
open. Task 20 uses one dedicated branch/PR, with dedicated commits for its
subtasks. Execution of this plan remains gated on the complete release rehearsal
and the owner's cutover decision; completing 20.2 does not complete Task 20.

## Exact inventory

[legacy-removal-manifest.json](legacy-removal-manifest.json) records 440 tracked
paths at the integration commit identified by `inventory_commit`, with Git blob,
mode, byte length and SHA-256. It exhaustively covers `internal/`, `frontend/`,
`build/`, `cmd/`, root Go files and the enumerated shared/legacy tooling paths.
It is a source snapshot, **not a deletion script or glob-based allowlist**.
Changes since that commit require a fresh inventory and review before removal.
Anything unlisted stays. No command in this document removes files.

| Group | Paths | Later disposition |
| --- | ---: | --- |
| Legacy shell | 24 | Archive the complete compatibility release, then remove Go/Wails entrypoints and shell tests after all gates. |
| Legacy backend | 232 | Preserve both oracle and current bridge sources and detach their consumers before removing working runtime sources/tests. |
| Legacy frontend | 92 | Preserve source/provenance and historical UI evidence before removing React, generated Wails bindings, browser tests and Node tooling. |
| Legacy build | 21 | Retire obsolete Go/Wails package/smoke commands and pins after their consumers are replaced. |
| Retained inputs | 49 | Keep fixtures, generators, migration bytes, catalogs, casefold data, icons and references at their existing paths. |
| Shared integration | 22 | Edit relevant sections only; retain Rust verification, native qualification and recovery support. |

`archive_then_remove_after_gates` is a future disposition. Every such path
remains in this checkout. Preserve the selected complete source commit and its
history, dependency locks, build pins and source archive; verify reconstruction
before retiring the maintained working-tree copy. The inventory commit is **not**
the selected last compatible published Go release. Task 20.1 must identify that
release, exact downloadable bytes and verified recovery assets separately.

## Dependencies that must survive

The Task 20 PR must resolve these consumers explicitly before removal, with matching tests.
A Rust-only production package does not imply Rust-only qualification tooling.

| Retained input or dependency | Current consumer and required treatment |
| --- | --- |
| `internal/localization/locales/{en,fr}.json` | Rust localization tests in `crates/librairii-app/src/localization.rs` include the legacy catalogs. Keep the source catalogs and comparison. |
| `internal/database/migrations/*.sql` | `xtask/check_migration_bytes.py` and `crates/librairii-storage/tests/migrations.rs` compare all 18 exact migration files against Rust and preserved corpus bytes. Keep all three namespaces; do not normalize newlines or change `wails-go-sqlite`. |
| `internal/library/testdata/story_library_query_codec.json` | `crates/librairii-core/tests/query_urls.rs` includes it directly. Keep the golden corpus and provenance. |
| Other legacy `testdata/`, fixture generators, performance fixtures, frontend casefold/demo/parity inputs | Keep the original bytes and generation recipes. A retained Go generator may need the archived module's dependencies; preserving only its single file is insufficient to promise regeneration. Adapt a producer only in a separate reviewed, equivalent-output change. |
| `build/appicon.{png,svg}` | `scripts/native_release/build.py` packages them; packaged acceptance also reads the PNG. The `build/` directory is not wholly obsolete. |
| `build/linux/librairii-acceptance-file-manager.desktop` | Shared acceptance reference; keep it and the native `scripts/librairii-linux-file-manager-evidence` helper. |
| Frozen oracle fallback | `scripts/frozen_go.py` reads `preservation/oracle-source.json`; it uses `preservation/oracle-files/<path>.in` when present, otherwise the working `internal/`, `go.mod` and `go.sum`. Materialize every still-needed missing retained copy against the **original** oracle checksum before deleting its fallback. Never replace frozen inputs with newer bridge bytes or rewrite oracle pins. |
| Current compatibility bridge | `scripts/verify-upgrade-handover.py` currently copies tracked `internal/`, `go.mod`, `go.sum` to build the current bridge. Bind that route to a separately retained, exact compatible bridge module and verify its source/execution identity before retiring those paths. The frozen 73a93303 oracle is a different product and cannot stand in for the bridge. |
| Frozen differential/recovery tools | Keep `scripts/frozen_go.py`, `scripts/*-probe.go.in`, `scripts/storage-query-oracle.go.in`, `scripts/capture-storage-query-oracle.py`, all native handover tools and their fixtures. Go may remain a development-only oracle dependency after product retirement. |
| Preserved source derivatives | Keep `.config/upstream-sources*`, notices, `third_party/`, Rust compatibility source manifests and their `.go.in`/`.go.txt` probes. Extension-based cleanup would destroy provenance. |

Each manifest entry that belongs to the frozen oracle includes its original
checksum/length and an existing retained-copy path, or `null` if no copy existed
at inventory time. These annotations identify obligations; they do not certify
that every future oracle or bridge dependency has already been detached.

## Coordinated changes in the gated retirement work

1. Complete Task 20.1 and all manifest prerequisites. Record final package hashes,
   the signed Mac evidence, recovery assets, resolved P58 decision and the complete
   three-target release rehearsal. Keep historical failures and review verdicts.
2. Freeze and verify both source lineages above. Prove the frozen oracle and
   compatible bridge can still be reconstructed and exercised without relying on
   files proposed for deletion. Do not mutate the existing oracle or user data.
3. Refresh this exact inventory against the chosen retirement source. Reconcile
   every added/modified/deleted path, including modes, with independent review.
   Preserve full source history and archives; keep retained inputs in place unless
   a separately reviewed relocation updates every consumer and preserves bytes.
4. Update shared tooling in the same change as runtime removal: replace legacy
   Make targets with documented Rust setup/check/package commands; retire only
   browser/Go **product** jobs in `.github/workflows/ci.yml` and their path selector;
   retain native installed qualification. Remove the obsolete frontend npm update
   entry. Reassess the root gomod entry against the retained bridge/oracle module
   strategy; preserve GitHub Actions and Cargo dependency updates.
5. Keep `rust-quality.yml`'s Go bridge/handover rehearsal and Go setup until a
   reviewed retained-module route supplies equivalent evidence. Inspect
   `rust-deep.yml`, `rust-packages.yml`, historical `task2-qualification.yml`,
   `.gitignore` and `.gitattributes` for mixed responsibilities rather than deleting
   whole workflows or changing immutable byte policies. Update README, development,
   platform and transition instructions to match the final commands and rollback.
6. Execute Task 20.4 on clean checkouts after removal: locked Rust quality checks,
   source/notice/corpus/migration/localization checks, oracle/bridge regeneration
   and recovery, then required three-target native/package checks. Native execution
   remains on isolated CI hosts. Confirm there are no dangling source paths, and
   that runtime packages still need no Go/Node/browser/toolchain installation.
7. Obtain independent Standards and Spec review of the actual deletion diff and
   resulting evidence before promotion. Only then may Task 20.5's final audit and
   OpenSpec archive workflow proceed under its own authorization.

Historical ADRs, UI prototypes/screenshots, `preservation/`, `qualification/`,
`testdata/`, native fixtures/probes, licenses, source manifests and Git history
survive. The ignored OpenSpec repository and retained `target/` evidence are outside
this tracked deletion scope. All external libraries, profiles, WAL/database files,
managed archives, trash, journals, backups and credentials are outside it as well.
Do not use this manifest for cache cleanup, uninstall or user-data migration.

## Read-only inventory verification

From the repository root, the following checks the committed inventory against
Git objects without building, executing native code, extracting archives or
changing source files. It validates the inventory snapshot, not release readiness
or permission to remove anything. Re-run it during review; compare the selected
future retirement tree separately rather than assuming these hashes remain current.

```sh
python3 -I -B - <<'PY'
import hashlib, json, subprocess
from pathlib import Path
m = json.loads(Path('docs/rewrite/legacy-removal-manifest.json').read_text())
assert m['state'] == 'plan_only' and m['removal_authorized'] is False
base = m['inventory_commit']
assert subprocess.check_output(['git', 'rev-parse', base + '^{tree}'], text=True).strip() == m['inventory_tree']
tracked = {}
for item in subprocess.check_output(['git', 'ls-tree', '-rz', base]).split(b'\0'):
    if item:
        meta, path = item.split(b'\t')
        mode, kind, blob = meta.decode().split()
        tracked[path.decode()] = (mode, kind, blob)
rows = [r for group in m['groups'] for r in group['files']]
paths = [r['path'] for r in rows]
assert len(paths) == len(set(paths))
scope = m['scope']
expected = {p for p in tracked if p.startswith(tuple(scope['complete_tracked_prefixes'])) or ('/' not in p and p.endswith('.go'))}
expected.update(scope['additional_exact_paths'])
assert set(paths) == expected
for r in rows:
    assert tracked[r['path']] == (r['mode'], 'blob', r['git_blob'])
    body = subprocess.check_output(['git', 'cat-file', 'blob', r['git_blob']])
    assert len(body) == r['bytes'] and hashlib.sha256(body).hexdigest() == r['sha256']
print(f'{len(rows)} exact tracked paths verified; removal remains unauthorized')
PY
```

## Dependency correction encountered during plan validation

The first Task20.2 CI source, `a18a1122d213c7b37364ad2f9c39540191c7fd0f`,
failed Linux's existing `cargo audit --deny warnings` gate because the registry
had yanked `async-executor 1.14.0`. This happened before workspace tests; later
speech steps lacked the unbuilt native executable and do not establish a speech
regression. The original failed log/archive and verdict remain retained.

The [upstream 1.14.1 fix](https://github.com/smol-rs/async-executor/pull/164)
reverts a state-lifetime optimization after a reported use-after-free when a
waker races executor destruction. The correction updates only that package's
locked version/checksum to the non-yanked 1.14.1 release. The registry dependency
and feature declarations are unchanged; no audit waiver, acceptance relaxation
or retirement action is added. The inventory remains explicitly bound to its
original integration snapshot. Fresh native checks must validate the corrected
graph; earlier observations do not qualify it or resolve P58.

## Linux pointer-latency investigation

Corrected dependency source `adcc898d637047037791d59ebea4bed14c215af4`
failed the unchanged Linux method2 pointer wall-time gate: desktop p95
103.715590 ms and loaded resource p95 121.320197 ms against 100 ms. Each cohort
retains 64 samples with four over 100 ms. The raw archive/provider SHA256 and
independent reviews remain preserved; this observation cannot be rescored.

The follow-up records pre-`xdotool` launch, process return and state-observation
monotonic timestamps alongside the existing uniquely numbered native callback.
All 64 clock handshakes occur after the complete input/navigation workload and
constrain one conservative clock-offset interval. The diagnostic distinguishes
process lifetime from callback bounds and callback-to-observation bounds.
Process return is not the event-post timestamp: process lifetime may overlap
application handling and must not be subtracted as pure posting overhead.
Callback-to-observation includes state publication and observer scheduling;
it does not distinguish those mechanisms by itself.

Acceptance remains method2's complete post-to-state wall interval, 64 input and
pointer samples, 20 cold navigation samples and unchanged 100/3000 ms limits.
Integer nanosecond timestamps avoid float clock subtraction; no sample is
trimmed or retried. Diagnostic callback bounds never replace acceptance values.
Raw cohorts are written before synchronization so a diagnostic error cannot
hide an already measured failure. Diagnostics explicitly carry
`joined_log_verified:false`: collection happens before shutdown. Offline review must bind every acknowledgement to the complete
joined `dispatch-events.json` using `dispatch.verify_events` before interpreting
the bounds. Positive worker join and fixture retirement are also required.

The hypotheses are posting/process delay, actual UI handling delay and
publication/observer delay. A fresh passing run alone proves none of them and
cannot close this failure or the separate historical P58 hold. Any correction
must follow the measured cause and retain both old and new evidence.

### First diagnostic observation

Source `4640a9638ee65c537829455c17667b0d2b3c81bf` completed all ten Linux
cohorts: 640 keyboard samples, 640 pointer samples, 200 cold completions and 640
clock handshakes. Both independent reviews matched every measured acknowledgement
to the 1,910 joined native events and verified positive cleanup, source/compiler
identity, fixture pins and archive seals. The original full scale audit passes.

Pointer wall p95 ranges 17.439892–24.214861 ms; conservative callback-upper p95
ranges 3.931608–9.552520 ms. No individual keyboard or pointer sample exceeds 100 ms.
The runner reports AMD EPYC 9V45, while the failed source reported EPYC 9V74.
This is not a matched-host before/after comparison. The observation verifies
that the diagnostics work, but supplies no slow event and proves no historical
cause or runtime correction. The original 103.715590/121.320197 ms failures remain
failed; neither they nor P58 are closed by this passing observation.

Raw [artifact 13215372848](https://github.com/01max/librairii/actions/runs/41601327214/artifacts/13215372848)
is retained locally under `target/task20-linux-latency/scale-linux.zip`, with
provider SHA-256 `35b45cbd9a2ee4502c17e69595487f4ed18b114ecb994153a77b3e2320493d88`.
The source-bound offline receipt inventories every expected cohort, including
missing or refused diagnostics, and never upgrades diagnostic bounds into
acceptance measurements.

## Storage-handover readiness correction

Diagnostic source `4640a9638ee65c537829455c17667b0d2b3c81bf` passed the ARM
workspace and native UI checks but failed the separate Go/Rust storage rehearsal:
the ready-file name existed while its payload was still empty. The Go helper
creates the file before writing `go-flock-owned\n`; existence alone therefore
races publication of the readiness payload. The failed archive and full log
remain retained with their provider identity.

A deterministic headless regression reproduces the empty/partial-write window.
The rehearsal now waits for the exact complete payload while checking owner
liveness and the original ten-second deadline, including after its final owner
observation. Wrong payloads, owner exit and late readiness still fail. Five
std-only Rust tests exercise this shared rehearsal function and run before the
real CI handover. The real cross-language lock exclusion and post-kill reopen
assertions remain unchanged; the marker is never substituted for those checks.

## Intel scale failure retained from the diagnostic run

The same source `4640a9638ee65c537829455c17667b0d2b3c81bf` failed the loaded
`gpu-api-2` resource cohort on Intel macOS. Its existing 240-second overall
observer watchdog expired before the Swift helper's Home-restoration wait ended.
The last state already selected index zero but was not settled: displayed
revision 354, pending revision 355. This establishes workload deadline exhaustion,
not a specific UI, scheduler, renderer or state-publication cause. No deadline or
acceptance budget is changed.

The shutdown record reports no worker shutdown error, but the success protocol
did not complete: the report records `workers_joined:false` and
`fixture_retired:false`, and the supervisor retained the fixture. The final
joined event log and complete helper sample report are absent. Earlier completed
cohorts cannot qualify this incomplete resource run or supply its missing samples.
Both independent reviews confirm that Intel scale remains failed, independently
of the completed Linux diagnostic observation and the storage-readiness fix.

Raw [artifact 13217332309](https://github.com/01max/librairii/actions/runs/41601327214/artifacts/13217332309)
and the primary full job log are retained under `target/task20-linux-latency/`.
The archive SHA-256 is
`dfdf2a1e430c0966556e3ba0e08e1dde7fd386a13d7fb0a473424dbbf726c67f`.
Historical latency failures, P58 and Task 20 release/cutover gates remain open.
