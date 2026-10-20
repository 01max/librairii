# Section 3: workspace and quality tooling

Owner: librairii-5. Scope: infrastructure beside the working Go/React product.
Parity P57/P59 foundations only; no product workflow or platform qualification.
The separate OpenSpec repository is read-only here and never nested into this tree.

## Plan and commit ledger

Each implementation commit carries `OpenSpec-Task: 3.N`. Full original SHAs are
retained here as subsequent subtasks land; use `git log --format=full` to verify.
Review fixes are separate commits. Section 3.9 stays pending independent AO review
of the final head and clean-checkout checks; unavailable target evidence is incomplete.

| Subtask | Commit | Evidence |
| --- | --- | --- |
| 3.1 | d289e5fa28ecfba3dc697e86c0a120694a12e325 | Exact stable 1.99.1, edition 2024/resolver 3, locked workspace build |
| 3.2 | 886b07b828663368f8fcf7cc2e5d990d41f92398 | Typed identity bytes/ranges, distinct group scope, unapproved review default, typed errors and compile-fail boundary |
| 3.3 | ec318f5e95ed067ac5f089f89f95c0f7b7f62734 | Stable rustfmt and Clippy, compiler warnings denied in checks |
| 3.4 | f0c19210b6efe57fe4824797cb24c894d9d44c53 | Owned fresh roots, missing/external/swapped/symlink rejection, loopback HTTP bytes; nextest CI reports and separate doctests |
| 3.5 | 586ff2b16a322155fab2c2fa0551c47dac27f706 | Locked check facade, dependency direction and refusing desktop process test |
| 3.6 | 3a385ea38986b1d3866395d5c5ca09030e580c88 | Native four-target CI definition; actual runs and release desktop evidence pending |
| 3.7 | b0fa2b536b89cd7c0a483677ac5f8cf68a4fdcd3 | deny/audit pass; exact source/license/duplicate policy, expiry tests and four-target inventory with upstream notice files |
| 3.8 | Pending commit | 18 nextest tests, independent fuzz lock/toolchain and bounded run, stable coverage and actual Criterion measurements |
| 3.9 | Pending | Independent review and clean-checkout/native target evidence |

## Baseline pins and boundaries

Rust 1.99.1 was verified against the official [release announcement](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/)
and installed through rustup. Normal builds require this stable pin, never nightly.
Workspace MSRV deliberately matches the tested compiler. The root lockfile covers
only the maintained workspace; qualification/section-2 and fuzz are separate graphs.
No GUI toolkit, SQLite, HTTP or archive library is selected by these skeletons.

Dependency direction: UI → app → core. Storage/adapters → app/core; desktop composes
the four outer boundaries. Testkit is synthetic support only. Core cannot import
application, persistence, network or GUI types. Empty module boundaries implement
no product behavior. The desktop executable deliberately exits unsuccessfully
before any data access; it is not an application or packaging acceptance claim.

Typed foundations are intentionally not persisted codecs or the reconciled product contract.
No new repository/product license is set; distribution decisions remain pending.

## Lint policy

Every member inherits compiler and Clippy lints. Safe Rust is enforced (`forbid`);
future OS FFI needs its own deliberately reviewed policy change. Ordinary Clippy
warnings are checked with `-D warnings`. Product/test code returns errors instead
of unwrap, expect, panic, todo or unimplemented. Pedantic/restriction groups are
not enabled wholesale. No generated/vendor exclusions or lint exceptions exist.
Any future exception must have a reason next to a narrow `#[expect(...)]`.
Native matrix jobs compile their actual target-specific modules; a host-only pass
never establishes the other targets. The current skeleton has no native FFI.

## Tests and pinned runner

Install `cargo-nextest` with `cargo install --locked --version 0.9.148 cargo-nextest`.
All helper versions live in `.config/rust-tools.toml`, verified against official
crates.io releases and the section2 ADR; upgrades require review. Routine tests:
`cargo nextest run --workspace --locked` (CI adds `--profile ci`), then separately
`cargo test --workspace --doc --locked`. CI JUnit is `target/nextest/ci/junit.xml`.
Retries are disabled. Test roots must come from TestRoots and be explicitly
validated; arbitrary production library/profile paths are never discovered or
accepted. Helpers never mutate process-global environment. LocalEndpoint accepts
only numeric loopback/nonzero-port addresses and performs no DNS resolution.

## Contributor check and editor setup

Use rustup with `rust-toolchain.toml`; install declared components, then
`cargo xtask tools` for exact locked helper installs. Run `cargo xtask check`
(or `make check-rust`). It fails on tool-version mismatch, reversed dependency
direction, unlocked graphs, build, formatting, Clippy, nextest or doctest failure.
`make check` remains the Go/React oracle suite and must also pass. Future product
fixtures/UI state tests must be added cumulatively; current typed foundations
and the refusing desktop process test do not claim those workflows exist.

Rust Analyzer: open the repository root, use the pinned rustup compiler, enable
`check.command = "clippy"`, and `cargo.allTargets = true`. Do not enable
`cargo.allFeatures` or add a GUI renderer until qualification. No extension is
required to build; `cargo fmt` provides save formatting. Editor-generated files
are not checked in.

`.config/rust-checks.toml` enumerates the four maintained native configurations,
currently empty/default features for the foundations only. There are no shipping
GUI combinations yet. Each native host runs all workspace tests/targets and
separate doctests; do not relabel cross-compilation as native execution evidence.

## Native CI and prerequisites

The additive Rust workflow always runs all workspace checks, including previous
Rust tests, on the four configurations. No path filters or continue-on-error.
A required aggregate refuses failed, cancelled and skipped matrix outcomes.
Official [runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
were checked on 2026-10-20: macos-15 is ARM, macos-15-intel is Intel,
ubuntu-24.04 is x64, windows-2025 is x64. Each job asserts the actual rustc host.
Standard hosted CI headless execution does not establish real desktop/Intel GUI,
assistive technology, GPU/software, signing, installation or packaged workflows.
Missing physical host evidence from section2 remains incomplete.

Prerequisites: macOS15 Xcode/Command Line Tools SDK and linker (deployment15.0);
Ubuntu24.04 gcc/binutils/glibc and pkg-config; Windows x64 MSVC Build Tools and
Windows SDK. No GUI/native-library requirements exist in this graph yet. Toolkit,
SQLite/archive and renderer prerequisites must be added after qualification.
Ubuntu installs reuse the signed official archive helper from merged support PR40
through integration ancestry. Its exact head3438bd9 passed native AO review and
Linux39439305675 plus macOS39439305577 CI; the runner-feed failure was intermittent
environment availability, not an application regression.
No paid runner/tool purchases are introduced.

Action revisions: official actions/checkout3d3c42e5 (v7.0.1) and
upload-artifactcf430e03 (v7.0.2), verified upstream signed commit metadata and
Node24 action entry points on 2026-10-20. Checkout credentials are not persisted;
workflow permissions are contents:read, no signing or deployment secrets.

## Dependencies and notices

`cargo xtask check` runs pinned cargo-deny0.20.2 plus cargo-audit0.22.2 with warnings
denied. `deny.toml` enumerates all four target graphs, denies unknown git/registry
sources, wildcard versions and duplicate releases, and allows only the currently
needed upstream MIT/Apache-2.0/Unicode-3.0 licenses, plus NCSA for the separate fuzz runtime. No advisory/license/duplicate
exceptions exist. Unpublished local crates are excluded only from license checks:
the product license is undecided, not implicitly inherited from dependencies.
All upstream development/benchmark dependencies remain checked.

Every future suppression must exactly match a record in
`.config/dependency-exceptions.toml`: kind, exact selector, owner, rationale,
HTTPS review/issue and real UTC expiry. Missing/duplicate/expired records fail;
expiry day fails. Broad subtree suppression is forbidden. Native libraries and
assets require their own reviewed notice/license obligations; a permissive wrapper
does not cover them. Toolkit/license selection remains gated.

`cargo xtask inventory` emits target/quality/dependency-inventory.json with source,
compiler, exact registry versions/checksums/features/dependency edges for all four
resolved graphs. target/quality/notices contains copied upstream LICENSE/COPYING/
NOTICE files and an index; missing upstream notice files fail generation. This is
a dependency inventory, not a complete release SBOM or distribution clearance.
Release packaging must include chosen toolkit/renderer/SQLite/assets/font terms
and inventory for final shipped bytes. Helper binaries are tools installed from
pinned upstream locked graphs, not linked into the desktop executable.

Dependabot adds root and fuzz Cargo graph proposals alongside unchanged Go/npm/
action updates. Review compiler/helper pins explicitly; archive/HTTP/SQLite/toolkit
updates must rerun the retained corpus, feature/native and licensing gates. No
auto-merge is enabled and no update is qualification by itself.

## Focused advanced checks (3.8)

Pinned proptest1.12.0 runs 256 cases per property for identity round trips,
exact checksum acceptance and distinct Collection/Shelf scope. Retain minimized
proptest regression seeds beside the failing integration test when found. The
reviewed inline insta1.49.0 snapshot is an internal boundary report, never an
invented product golden. FailAfter injects deterministic partial writes into a
real temporary destination; tests assert exact retained source/partial bytes.
It is a helper, not an implementation of durable custody/recovery.

The independent fuzz workspace has its own Cargo.lock, exact nightly2026-10-19
(rustc1.101.0-nightly b6eeb93c9), libfuzzer-sys0.4.13 and cargo-fuzz0.13.2.
`cargo xtask fuzz` copies the four retained synthetic seeds into a generated
working corpus, resolves locked metadata, runs offline for30s/max128bytes/512MiB
RSS with ASan, then rejects lock changes. Normal checks stay on stable1.99.1.
The installed tool version was verified through official rustup metadata; no
moving nightly channel is used. Initial ARM run completed30,206,540 executions
in31s, no crash, unchanged lock. Future runs may discover new coverage/failures;
minimize and retain crash seeds with a behavioral regression assertion.
Archive/JSON/query parser fuzz cases remain unimplemented until the actual ports
exist; these identity checks cannot qualify those failure classes.

`cargo xtask coverage` uses pinned cargo-llvm-cov0.9.1 and llvm-tools-preview on
stable, writes LCOV and an HTML report under target/quality, and records source/
compiler/OS/architecture/fixture/hardware context. It runs the full nextest suite;
doctests remain separately required by check. No blanket coverage threshold:
initial report has identity100% line coverage and testkit96.86%, but tool CLI
orchestration is exercised by real commands rather than its test binary. Remaining
message mappings, helper error branches and Windows reparse behavior need explicit
evidence; stable coverage does not claim nightly branch coverage or future
custody/migration/recovery/cancellation coverage.

`cargo xtask bench --smoke` exercises the Criterion0.8.2 harness; `cargo xtask
bench` records100 measured samples after3s warmup for each fixed identity parser.
Set LIBRAIRII_BENCH_HARDWARE to an actual machine description; absent identification
is labelled diagnostic only. Initial macOS15.7.7 Apple M4 Pro/48GiB stable1.99.1
measurements: valid checksum19.759–19.875ns, multibyte rejection1.408–1.416ns,
canonical UUID6.487–6.910ns. These synthetic measurements are not the retained
5,000-story query/archive/Shelf budgets or native UI performance qualification.
The focused CI lane runs stable coverage, benchmark harness and bounded fuzzing;
scheduled/manual runs additionally measure diagnostic benchmarks. All original
native/Go/React suites remain cumulative and required independently.

Both root and fuzz lockfiles receive deny/audit and per-target inventories.
Bundled libFuzzer source headers differ from its README's short license label:
the checked-in supplemental upstream LLVM/compiler-rt notices preserve the full
Apache-with-LLVM-exception and historical text at the exact vendored revision.
The benchmark-only anes package omits notices; its exact published VCS revision's
MIT/Apache files are preserved with provenance. These are dependency notices,
not a new product license; no fuzz/benchmark runtime is linked into the desktop.

## Evidence status

Local macOSARM: make check passed (all Go/vet/build/type/lint plus207frontend tests,
responsive/visual/accessibility/performance); Rust foundation check passed18tests,
zero skipped, separate compile-fail doctest, locked stable build/fmt/Clippy and
both graph policies. Coverage, benchmark smoke/measured runs and bounded fuzz ran.
Native CI is in progress on draft PR41; no missing target is treated as passed.
Task1 reviewed contract/corpus admission and final clean-checkout checks remain
pending. Full desktop/assistive-tech/platform/package qualification remains
outside this infrastructure section. Section3.9 is not complete.
