# CI during the native rewrite

The active targets are Linux x64, macOS ARM64, and macOS Intel. Windows CI is
deferred. Its source, portability fixtures, and historical test identities remain
available for a later Windows qualification pass.

| Lane | When it runs | Work retained |
| --- | --- | --- |
| Rust native matrix | Every PR, main/integration push, or manual run | Complete `cargo xtask check` on all three native hosts |
| Rust required targets | Every native run, including failed/cancelled runs | Rejects any failed, skipped, or cancelled native matrix result |
| Rust native packages | Every PR, main/integration push, or manual run | Builds three matching-host candidates, then independently inspects/installs and exercises their actual GUI bytes |
| Rust scale and resilience | Every PR, main/integration push, or manual run | Retained backend, native input/completion, display and release-resource workloads on all three native targets |
| Task 2 qualification | Changes to `qualification/section-2/**` or its workflow; manual run | Supporting toolkit/native-host observations; historical spike results remain source-bound |
| Rust focused checks | Integration/main pushes; PRs changing the lane, toolchain/dependency pins, fuzz or benchmark harness; weekly/manual | Coverage, benchmark smoke, bounded fuzzing; measured benchmarks on weekly/manual runs |
| Optional paid Mac signing | Manual dispatch after trusted workflow registration and owner setup | Trusted signing/notarization/stapling, followed by credentialless inspection and full installed-GUI verification of the final signed bytes |

Routine changes run the complete Rust native suites. The Go/Wails product jobs
are retired. `ci.yml` retains only the manually selected native package rerun;
it requires the exact package run, source and independent checksum. CI wiring
changes exercise the affected lanes before merge. Integration pushes keep
focused checks active while the rewrite is developed separately from `main`.

Performance remains blocking on every PR, including the unchanged 100 ms native
dispatch and 3000 ms completion limits. A slower shared runner does not waive a
failure. Passing later observations do not rescore retained failures.

Native quality, scale and package workflows do not cancel an active run when a
new commit arrives. Their workers and owned volumes must finish cleanup first.
The concurrency group serializes replacements; superseded pending runs may still
be dropped by GitHub before they start.

Desktop scale records both renderer outcomes after a numerical failure when the
failed case positively joined its workers and retired its fixture. Unknown
cleanup stops further native execution. Any failed or unattempted renderer keeps
the manifest incomplete and the job red; the final manifest retains failure
messages and hashes for the files actually collected.

macOS relocation cleanup resolves the fresh owned image through `hdiutil info
-plist`, rather than relying on a mountpoint that may already have disappeared.
Only its uniquely identified GUID backing device may be detached. Success
requires both absence of that image from the attachment inventory and an
unmounted owned mountpoint. A successful command alone is insufficient, and a
failed command is accepted only when those disposal checks independently pass.
Filtered observations are retained as `native-relocation-disposal.json`; unknown
identity or disposal preserves the workspace and fails the job.
After all four permitted detach attempts fail with the busy status, the receipt
also retains one ten-second, read-only `lsof` diagnostic limited to that image
and the devices in its last matching inventory. Partial output, nonzero status,
missing-tool errors and timeouts remain diagnostic observations. They never
establish disposal or replace the original failure; no extra detach is attempted.

The [development guide](../development.md) documents the Rust-only contributor
commands. Frozen Go reference inputs are retained, but ordinary CI neither
installs Go/Node nor builds the old product.

Native package jobs build before installation jobs. Build/payload inspection,
actual installed GUI and optional paid signing are separate evidence gates.
Free ad-hoc Mac packages are the default and need no signing credentials. ARM and
Intel use separate matching hosts; Linux runs the installed workflows under
X11 and Wayland. The signing workflow permits only `main` or the integration
branch and requires the protected `native-macos-signing` environment. A PR job
does not receive signing credentials. See the
[platform matrix](../release-platforms.md) and
[protected signing setup](section-18.md#protected-mac-signing).

## Cache boundaries

The local `rust-environment` action restores Cargo dependencies/build outputs and
separately caches compiled helpers. Keys include the native OS/architecture,
helper group, exact compiler pins, and manifest/lockfile or helper-pin inputs.
Native checks install only nextest/deny/audit; focused checks install only
nextest/llvm-cov/fuzz. Restored helper versions are checked before use; missing
or mismatched binaries are installed with exact versions and locked dependencies.
Official remote actions are pinned to immutable commits; the local action comes
from the tested checkout.

All gates execute on every selected run. Cache paths exclude quality reports,
nextest execution/JUnit state, coverage reports, and benchmark evidence. Fresh
source/host evidence and reports are uploaded for the checked commit. Incremental
compilation is disabled in CI; Cargo validates cached compilation against current
sources. Cold or evicted caches still rebuild the helpers, so cache reuse reduces
repeat setup work without establishing a fixed runtime guarantee.

Historical four-platform foundation runs in [section 3](section-3.md) remain
evidence for their original commits, rather than current platform commitments.
Those original runs do not qualify later product bytes. Current GUI, package,
accessibility and physical-host evidence comes from the corresponding recorded
native runs; source, target, package and observer identities must stay bound.
