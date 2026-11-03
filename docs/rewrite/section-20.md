# Legacy runtime retirement plan

Task 20.2 prepares a removal manifest. It does **not** retire any runtime,
select a release candidate, approve cutover or complete Tasks 20.1/20.3–20.5.
Signing/final-byte qualification and the historical P58 performance hold remain
open. Task 20 uses one dedicated branch/PR, with dedicated commits for its
subtasks. Execution of this plan remains gated on the complete release rehearsal
and the owner's cutover decision; completing20.2 does not complete Task20.

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

The removal PR must resolve these consumers explicitly, with matching tests.
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

## Coordinated changes in the future retirement PR

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
103.715590ms and loaded resource p95 121.320197ms against100ms. Each cohort
retains64 samples with four over100ms. The raw archive/provider SHA256 and
independent reviews remain preserved; this observation cannot be rescored.

The follow-up records pre-`xdotool` launch, process return and state-observation
monotonic timestamps alongside the existing uniquely numbered native callback.
All64 clock handshakes occur after the complete input/navigation workload and
constrain one conservative clock-offset interval. The diagnostic distinguishes
process lifetime from callback bounds and callback-to-observation bounds.
Process return is not the event-post timestamp: process lifetime may overlap
application handling and must not be subtracted as pure posting overhead.
Callback-to-observation includes state publication and observer scheduling;
it does not distinguish those mechanisms by itself.

Acceptance remains method2's complete post-to-state wall interval,64 input and
pointer samples,20 cold navigation samples and unchanged100/3000ms limits.
Integer nanosecond timestamps avoid float clock subtraction; no sample is
trimmed or retried. Diagnostic callback bounds never replace acceptance values.
Raw cohorts are written before synchronization so a diagnostic error cannot
hide an already measured failure. The final joined native event log must be
checked against every diagnostic acknowledgement before interpreting results.

The hypotheses are posting/process delay, actual UI handling delay and
publication/observer delay. A fresh passing run alone proves none of them and
cannot close this failure or the separate historical P58 hold. Any correction
must follow the measured cause and retain both old and new evidence.
