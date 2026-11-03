# Rust-only runtime and historical retirement plan

**Current scope (2026-11-03):** the owner approved a fresh Rust app and retirement
of the old runtime without Go data migration or paid Mac signing. The
[fresh Rust contract](fresh-rust-contract.md) supersedes the original bridge,
signed-release and legacy cutover prerequisites below. The original inventory
and observations remain historical evidence; the Task 20 implementation records
its actual removals separately.

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
Independent review and current-source three-target CI remain pending.

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
