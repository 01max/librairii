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
| 3.8 | 1c0b6b65c40a2ebb7b5e39771a91589463fb43c0 | 18 nextest tests, independent fuzz lock/toolchain and bounded run, stable coverage and actual Criterion measurements |
| 3.9 | Pending gate; separate evidence commit | Cumulative discovery guard and provisional clean-checkout evidence below; independent final review/native/corpus admission still required |

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

Every member inherits compiler and Clippy lints. Safe Rust is the workspace default (`deny`); core/application/UI/testkit/desktop/
xtask additionally use crate-level `forbid`. Storage/adapters have the narrow
reviewed FFI route documented below. Ordinary Clippy
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

## Storage handoff seams

LibraryId/StoryUuid/JobId Display produces canonical UUID text for diagnostics,
not a persisted encoding. Storage must retain original legacy spelling/file bytes
alongside typed identity and must never rewrite identities just because Display
canonicalizes them. No serialization or migration codec is supplied by task3.
Section4 owns product root authority, profiles, locks and database behavior;
TestRoots only authorizes its own fresh synthetic directories for tests.
Root Cargo.toml/Cargo.lock stays with task3 until integration; section4 must
coordinate its exact rusqlite pin/feature addition and lock resolution rather
than overwrite concurrent graph changes. Task2 candidate is rusqlite0.40.2 with
bundled SQLite/backup; qualification remains provisional.

Workspace unsafe_code=deny permits a deliberate isolated FFI implementation in
storage/adapters platform modules. Core/application and other pure boundaries
remain crate-level forbid. Prefer maintained safe OS wrappers first. Necessary
unsafe requires a narrowly scoped `#[expect(unsafe_code, reason = "specific OS
invariant and evidence reference")]` at the individual function/block boundary,
SAFETY comments, public unsafe API safety docs, ownership/alias/lifetime/thread
invariants, and positive/failure/race tests on the actual cfg target. Clippy denies
undocumented unsafe blocks and missing safety docs. Never allow unsafe for a crate,
whole platform module, unrelated logic or every lint. A new FFI change needs
independent review of that target's code and evidence; there is no authored unsafe
or exception in the current workspace. Proposed section4 paths are storage
layout/identity/profile/lock/platform modules, pending direct owner confirmation.

## Section 3.9: pending admission gate

**PENDING.** This evidence commit records checks and enforces cumulative discovery;
it does not complete independent review, task1 admission or missing native checks.
No external OpenSpec checkbox is marked complete by this session.

An independent `git clone --no-local --single-branch` at
1c0b6b65c40a2ebb7b5e39771a91589463fb43c0 (tree49453e50ef362b4c02559a88644a26c6998182bc)
ran cargo xtask check and make check successfully on macOS15.7.7 ARM/M4Pro/48GiB,
with stable1.99.1, declared pinned helpers and Node24.15.0. The clone began without
build outputs/node_modules; registry/tool/browser caches were available on the
host, so this is clean-checkout evidence, not a clean-machine installation claim.
Its tracked status remained clean. Rust ran18tests+onecompile-fail doctest at that
head; make check ran207frontend tests plus responsive/visual/accessibility and
performance as well as the full retained Go/vet/type/lint/build checks.
Later safety fixes add one exact-selector test and one root-lifetime doctest;
the discovery guard adds one negative test: current local20tests+2doctests pass.
All earlier assertions remain. Native CI on the earlier head remains provisional;
a changed head requires renewed exact-head CI/review and cumulative checks.

`.config/rust-test-baseline.json` records the required foundation tests by suite
and name. `xtask check` validates pinned nextest discovery before running the full
workspace, rejects missing, ignored or filtered assertions, and saves discovery
with CI evidence. Unix symlink tests apply only to the three Unix targets; Windows
has19current tests and the Unix hosts20. This explicit cfg scope is not a Windows
reparse/locking pass. Additions remain cumulative; replacing a scaffold behavior
with its real port requires an independently reviewed baseline replacement proving
the intended invariant. The separate doctest step remains mandatory.

Remaining admission work: merge latest integration including independently reviewed
task1 contract/corpus; wire task1's cross-platform read-only preservation/check.py
integrity/semantic checks into ordinary xtask (no Windows Go desktop dependency);
keep oracle regeneration/replay separate; rerun clean-checkout/current cumulative
Go+Rust/fixture/native CI; resolve native AO review findings in separate commits;
obtain independent approval on the exact final head and retain review/CI identities.
Missing, skipped, failed or cancelled required checks are incomplete. No merge or
production qualification is authorized by these local results.

Additional focused commits:577f9755255411de3e1520c66eac3fc1e0106d29 bounds authorized
root lifetimes and exact exception selectors;556d295ff674900f93e0b960c22ab464caef6952
records raw UUID and narrow reviewed FFI seams. Original subtask/fix SHAs are never
rewritten. The evidence commit's own SHA is available from its OpenSpec-Task3.9
trailer and will be appended to this ledger by the next evidence update.
