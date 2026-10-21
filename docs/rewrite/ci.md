# CI during the native rewrite

The active targets are Linux x64, macOS ARM64, and macOS Intel. Windows CI is
deferred. Its source, portability fixtures, and historical test identities remain
available for a later Windows qualification pass.

| Lane | When it runs | Work retained |
| --- | --- | --- |
| Rust native matrix | Every PR, main/integration push, or manual run | Complete `cargo xtask check` on all three native hosts |
| Rust required targets | Every native run, including failed/cancelled runs | Rejects any failed, skipped, or cancelled native matrix result |
| Rust focused checks | Integration/main pushes; PRs changing the lane, toolchain/dependency pins, fuzz or benchmark harness; weekly/manual | Coverage, benchmark smoke, bounded fuzzing; measured benchmarks on weekly/manual runs |
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
with that workflow's manual dispatch.

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
This maintenance change does not advance OpenSpec task 9 or qualify desktop GUI,
packaging, accessibility, or physical-host behavior for the Rust app.
