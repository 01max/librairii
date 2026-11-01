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
| Protected Mac signing | Manual dispatch after trusted workflow registration and owner setup | Trusted signing/notarization/stapling, followed by credentialless inspection and full installed-GUI verification of the final signed bytes |
| Go/Wails selector | Every PR, main push, or manual run | Tests the path selector, then reads the complete local Git diff |
| Go/Wails verification and Linux release evidence | Relevant changes, unknown paths, unavailable history, or manual run | Existing macOS `make check`/build and Linux packaged acceptance/archive checks |

Routine Rust changes therefore run three complete native suites, without the
additional Go/Wails builds or focused analysis job. CI wiring changes exercise
the affected lanes before merge. Scheduled GitHub workflows use the default
branch, currently Go `main`; the explicit integration push trigger keeps focused
checks active during the rewrite.

The selector in `.github/scripts/legacy-ci-changes.py` skips Go/Wails only for
changes entirely within the Rust workspace/configuration or rewrite documents.
Everything else runs those suites. It reads NUL-delimited Git paths, includes
deletions and both sides of renames, and has no API path-list limit. A shallow
checkout that lacks the comparison commit runs the suites. Go/Wails is still the
working app until the cutover; its jobs live together in `ci.yml` and can be forced
with that workflow's manual dispatch. The native Rust contributor commands and
the retained Go tooling requirements are in the
[development guide](../development.md).

Native package jobs build before installation jobs. Build/payload inspection,
actual installed GUI and public signing are separate evidence gates. ARM and
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
