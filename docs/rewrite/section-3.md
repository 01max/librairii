# Section 3: workspace and quality tooling

Historical foundation report. Statements below about an unimplemented GUI,
missing native dependencies and foundation-only features describe the recorded
section-3 revisions. Current setup and commands are in the
[native development guide](../development.md); see the
[runtime architecture](../architecture/0005-rust-native-desktop.md) and
[platform matrix](../release-platforms.md) for the implemented candidate.

Owner: librairii-5. Scope: infrastructure beside the working Go/React product.
Parity P57/P59 foundations only; no product workflow or platform qualification.
The separate OpenSpec repository is read-only here and never nested into this tree.

## Plan and commit ledger

Each implementation commit carries `OpenSpec-Task: 3.N`. Full original SHAs are
retained here as subsequent subtasks land; use `git log --format=full` to verify.
Review fixes are separate commits. Admission requires independent AO review and
all provider checks on the resulting final head; unavailable target evidence is
incomplete. Local audit completion never substitutes for those external gates.

| Subtask | Commit | Evidence |
| --- | --- | --- |
| 3.1 | d289e5fa28ecfba3dc697e86c0a120694a12e325 | Exact stable 1.99.1, edition 2024/resolver 3, locked workspace build |
| 3.2 | 886b07b828663368f8fcf7cc2e5d990d41f92398 | Typed identity bytes/ranges, distinct group scope, unapproved review default, typed errors and compile-fail boundary |
| 3.3 | ec318f5e95ed067ac5f089f89f95c0f7b7f62734 | Stable rustfmt and Clippy, compiler warnings denied in checks |
| 3.4 | f0c19210b6efe57fe4824797cb24c894d9d44c53 | Owned fresh roots, missing/external/swapped/symlink rejection, loopback HTTP bytes; nextest CI reports and separate doctests |
| 3.5 | 586ff2b16a322155fab2c2fa0551c47dac27f706 | Locked check facade, dependency direction and refusing desktop process test |
| 3.6 | 3a385ea38986b1d3866395d5c5ca09030e580c88 | Four native foundation targets passed at952a923; changed final head requires fresh CI; release desktop evidence remains pending |
| 3.7 | b0fa2b536b89cd7c0a483677ac5f8cf68a4fdcd3 | deny/audit pass; exact source/license/duplicate policy, expiry tests and four-target inventory with upstream notice files |
| 3.8 | 1c0b6b65c40a2ebb7b5e39771a91589463fb43c0 | 18 nextest tests, independent fuzz lock/toolchain and bounded run, stable coverage and actual Criterion measurements |
| 3.9 | b9bfa1a04a35854097bb03e8a67c1869643e991d;13820ffb3ef3203c323f8a2e041926426a97f2bf | Cumulative discovery and admitted preservation gate; dedicated completion evidence below; final-head external admission remains required |

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
direction, preservation integrity/witness failure, unlocked graphs, build,
formatting, Clippy, nextest or doctest failure.
`make check` remains the Go/React oracle suite and must also pass. Future product
fixtures/UI state tests must be added cumulatively; current typed foundations
and the refusing desktop process test do not claim those workflows exist.

Rust Analyzer: open the repository root, use the pinned rustup compiler, enable
`check.command = "clippy"`, and `cargo.allTargets = true`. Do not enable
`cargo.allFeatures` or add a GUI renderer until qualification. No extension is
required to build; `cargo fmt` provides save formatting. Editor-generated files
are not checked in.

`.config/rust-checks.toml` enumerates the three active Linux/macOS native configurations,
currently empty/default features for the foundations only. There are no shipping
GUI combinations yet. Each native host runs all workspace tests/targets and
separate doctests; do not relabel cross-compilation as native execution evidence.

## Native CI and prerequisites

The additive Rust workflow always runs all workspace checks, including previous
Rust tests, on the three active configurations. No path filters or continue-on-error.
The current scheduling/cache policy is in [CI during the native rewrite](ci.md);
the four-platform measurements below record historical foundation evidence.
A required aggregate refuses failed, cancelled and skipped matrix outcomes.
Official [runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
were checked on 2026-10-20: macos-15 is ARM, macos-15-intel is Intel,
ubuntu-24.04 is x64. Windows is deferred. Each job asserts the actual rustc host.
Standard hosted CI headless execution does not establish real desktop/Intel GUI,
assistive technology, GPU/software, signing, installation or packaged workflows.
Missing physical host evidence from section2 remains incomplete.

Prerequisites: macOS15 Xcode/Command Line Tools SDK and linker (deployment15.0);
Ubuntu24.04 gcc/binutils/glibc and pkg-config. No GUI/native-library requirements
exist in this graph yet. Toolkit,
SQLite/archive and renderer prerequisites must be added after qualification.
Every active host also needs `python3` with its standard-library sqlite3 module.
The former Windows PowerShell workflow is deferred with that target.
Ubuntu installs reuse the signed official archive helper from merged support PR40
through integration ancestry. Its exact head3438bd9 passed native AO review and
Linux39439305675 plus macOS39439305577 CI; the runner-feed failure was intermittent
environment availability, not an application regression.
No paid runner/tool purchases are introduced.

## Admitted preservation corpus

Task1 PR39's reviewed head1cd22443cae3b548bb7f9b3fb2435dcd73c3c65c was admitted
through integration merge9a6c1c116d9df726a061ad6aac3955df4ed41263. Ordinary
`cargo xtask check` now requires its committed `preservation/check.py` and
146-file hashed corpus (plus manifest), including actual Go-produced recovery
outcomes for three positive and two negative cases. No task1 file is changed.
The checker verifies exact bytes/inventory, producer hashes, read-only SQLite
integrity and independent semantic witnesses for the retained behavior. These
are Go-produced expectations, not implemented Rust product differential tests.

`.config/rust-preservation-baseline.json` pins the admitted oracle, manifest and
checker hashes. Missing, changed or silently reduced baseline inputs fail before
execution; a later intentional corpus/checker update must update these pins under
review while preserving prior assertions. Python runs with `-I -B -X utf8` so
environment optimization cannot disable assertions, no bytecode is written, and
native Windows preserves the Unicode witnesses. The generated report is
`target/quality/preservation.json`, retained in every native CI artifact.
Synthetic temporary-root regressions verify missing/changed inputs and wrong
oracle/inventory fail. There is no if-present or platform skip for preservation.
The Rust regression entry uses a strict Python runner with individually retained
identities in `.config/rust-python-test-baseline.json`. It rejects removed or
duplicate tests, skips, expected failures and incomplete execution. Behavioral
runner regressions execute synthetic passing/skipped/xfail/failing suites and
assert the actual accepted/rejected result; no outer required assertion is skipped.

The root checkout rule `preservation/** -text` protects every admitted checker,
producer, manifest and corpus byte from Git newline conversion, including under
Windows `core.autocrlf=true`. Exact hashes stay unchanged; no normalization is
performed by the gate. The former Windows CI additionally cloned a fresh independent
checkout with conversion explicitly enabled before cumulative checks. That job is
now deferred; the checkout protection and cross-platform Python regression tests
remain. Its earlier reports are historical evidence for the original commits.

Oracle regeneration and byte-identical export/recovery replay remain separate
task1 procedures. They use an independent disposable Go checkout and synthetic
roots; ordinary native Rust checks never execute the generator, discover a user
root, or contact product endpoints. This hook cannot qualify future Rust ports.

Action revisions: official actions/checkout3d3c42e5 (v7.0.1) and
upload-artifactcf430e03 (v7.0.2), verified upstream signed commit metadata and
Node24 action entry points on 2026-10-20. Checkout credentials are not persisted;
workflow permissions are contents:read, no signing or deployment secrets.

## Dependencies and notices

`cargo xtask check` runs pinned cargo-deny0.20.2 plus cargo-audit0.22.2 with warnings
denied. `deny.toml` enumerates the three active target graphs, denies unknown git/registry
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
compiler, exact registry versions/checksums/features/dependency edges for the three
active resolved graphs. target/quality/notices contains copied upstream LICENSE/COPYING/
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
The focused CI lane runs stable coverage, benchmark harness and bounded fuzzing
after integration pushes and on PRs changing its wiring/toolchain/harnesses;
scheduled/manual runs additionally measure diagnostic benchmarks. All active
native suites remain cumulative; Go/React suites run for relevant changes as
described in [the current CI policy](ci.md).

Both root and fuzz lockfiles receive deny/audit and per-target inventories.
Bundled libFuzzer source headers differ from its README's short license label:
the checked-in supplemental upstream LLVM/compiler-rt notices preserve the full
Apache-with-LLVM-exception and historical text at the exact vendored revision.
The benchmark-only anes package omits notices; its exact published VCS revision's
MIT/Apache files are preserved with provenance. These are dependency notices,
not a new product license; no fuzz/benchmark runtime is linked into the desktop.

## Storage handoff seams

LibraryId/StoryUuid/JobId Display produces canonical UUID text for diagnostics,
not a persisted encoding. Storage must retain original legacy spelling/file bytes
alongside typed identity and must never rewrite identities just because Display
canonicalizes them. No serialization or migration codec is supplied by task3.
Section4 owns product root authority, profiles, locks and database behavior;
TestRoots only authorizes its own fresh synthetic directories for tests.
Root Cargo.toml/Cargo.lock stays with task3 until integration; section4 must
coordinate its exact rusqlite pin/feature addition and lock resolution rather
than overwrite concurrent graph changes. The coordinated candidate is exact
rusqlite `=0.40.2`, `default-features=false`, features `bundled,backup`. Actual
bundled SQLite version and product behavior need observed evidence; qualification
remains provisional. This infrastructure commit adds no SQLite dependency.

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
or exception in the current workspace.

The final written path/interface agreement was sent to owners6 and2. Owner5 keeps
root Cargo/lock/toolchain/lints/xtask/CI, shared core IDs/errors, app shared ports,
testkit, non-storage crate roots/shells, and the initial storage Cargo.toml and
src/lib.rs shells until an explicit later handoff. Only after orchestrator release
and that handoff may owner6 implement this storage-only set:

```
crates/librairii-storage/Cargo.toml
crates/librairii-storage/src/lib.rs
crates/librairii-storage/src/{error,layout,identity,migrations,database,backup,profile,adoption,portability}.rs
crates/librairii-storage/src/platform/{mod,unix,macos,linux,windows,lock}.rs
crates/librairii-storage/migrations/001-018
crates/librairii-storage/tests/**
crates/librairii-storage/evidence.md
```

No adapters, generators/corpus, Go bridge17 or frozen index edits are included.
Shared ports/errors/dependency/lock changes coordinate through owner5. This
agreement and code readiness do not release section4; it remains read-only until
explicit orchestration after final foundation admission.

## Section 3.9: reproducibility and final admission

The admitted corpus is now mandatory. The completion evidence commit follows
the implementation commits and records this audit; its own SHA is retained in
the AO admission report (a commit cannot embed its own hash). The separate
OpenSpec checkout is not published or nested here. Final external admission
remains conditional on exact resulting-head review and all current-head CI.

An independent `git clone --no-local --single-branch` at
13820ffb3ef3203c323f8a2e041926426a97f2bf
(tree a8c64aeba5b41ee51289e46c48c47dbe1f0f2323) includes integration9a6c1c1
and the mandatory fixture hook. It began without build outputs/node_modules;
registry/tool/browser caches were available, so this is a clean-checkout audit,
not a clean-machine installation claim. Host: macOS15.7.7 ARM, AppleM4Pro/48GiB,
stable1.99.1, pinned helpers, Node24.15.0, Python3.14.7. Checks:

- `cargo xtask check`: locked workspace build, rustfmt, Clippy, deny/audit of
  root/fuzz graphs, inventory/notices, all146 hashed corpus files and semantic
  witnesses, 24 nextest tests with zero skipped, plus2 compile-fail doctests.
- `GOMAXPROCS=2 make check`: every retained Go/vet/build/type/lint check,
  207frontend tests, responsive/visual/accessibility and performance checks.
  No production roots or endpoints were used. Tracked checkout status is clean.
- A disposable copy of the actual admitted corpus accepts exact bytes, rejects
  changed/missing Unicode fixture bytes, and leaves all source corpus bytes intact.

The first default-parallel local make check failed existing settings-cleanup
subtests at their5s subprocess timeout. Focused runs reproduced the same failure
in the previously passing independent952a923 checkout. A bounded local Go process
budget of2 passed all four unchanged cases, then the full clean-checkout suite.
No test, assertion, timeout, application code or CI setting was weakened; this
host execution sensitivity remains part of the evidence rather than a hidden pass.

Prior independent clones at1c0b6b6 and952a923 also passed complete Rust and Go/React
checks. Their results are provenance only and do not approve subsequent changes.

`.config/rust-test-baseline.json` records the required foundation tests by suite
and name. `xtask check` validates pinned nextest discovery before running the full
workspace, rejects missing, ignored or filtered assertions, and saves discovery
with CI evidence. Unix symlink tests apply only to the three Unix targets; Windows
has23current tests and the Unix hosts24. This explicit cfg scope is not a Windows
reparse/locking pass. Additions remain cumulative; replacing a scaffold behavior
with its real port requires an independently reviewed baseline replacement proving
the intended invariant. The separate doctest step remains mandatory.

Native AO review approved952a923ffe4b6803b03deafb9c148ea9b184cf0f
(review5587047882, run6d44930f-e1a7-409c-92a3-f4257ffd76c0). Required expiry
canonicalization and portable UTC corrections, plus selected-nightly provenance,
were accepted. Both inline findings are resolved. Actual952a923 provider evidence:

| Check | Run / job | Result |
| --- | --- | --- |
| ARM native | 39444932520 /118704922879 | success |
| Intel native | 39444932520 /118704922805 | success |
| Linux native | 39444932520 /118704923129 | success |
| Windows/MSVC native PowerShell without GNU date | 39444932520 /118704922558 | success; check completed2026-10-20T15:22:54Z |
| Required native aggregate | 39444932520 /118716755501 | success |
| Coverage/benchmark/fuzz | 39444932754 /118704201154 | success |
| Retained Go/macOS | 39444932882 /118704202809 | success |
| Retained Linux release/package | 39444932613 /118704200489 | success |

Integration9a6c1c116d9df726a061ad6aac3955df4ed41263 also passed macOS39447665124
and Linux39447665711. These exact heads precede the new fixture hook/completion
commit: they cannot approve the changed final head. The AO final admission report
and provider state must retain the resulting final SHA, native review identity
and all fresh check identities. Missing, skipped, failed or cancelled required
jobs are incomplete. OS-specific inapplicable steps do not replace required jobs.
No physical Windows11/Intel GUI, assistive-tech, renderer, signing/packaging or
full Rust workflow qualification is established. The worker never merges PR41
or global PR37; owner2 checks final conditional admission.

Additional focused commits:577f9755255411de3e1520c66eac3fc1e0106d29 bounds authorized
root lifetimes and exact exception selectors;556d295ff674900f93e0b960c22ab464caef6952
records raw UUID and narrow reviewed FFI seams. Review corrections are
f2942806affe44ad457185672588e52a4fd21832 (canonical expiry),
411a29e77ad98d4a0365a6a6d0090a50ab970abb (in-process UTC/native shell),
8ce01fbc2dd24cd91cc49496d60c444ecad8a62e (actual selected-nightly compiler), and
952a923ffe4b6803b03deafb9c148ea9b184cf0f (cumulative review regressions).
CI concurrency fix9f91d3b94b8ff6ec2a32a5c494c4d4fea8bc0be0 preserves distinct
PR/ref checks. Original subtask/fix SHAs are never rewritten.
