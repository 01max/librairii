# Partial spike validation snapshot

2026-10-20, observer librairii-4. ARM host/SDK inventory: hosts.md.
Code tested at `954a951` (UI fix `ea9ad3a`); includes integration
`41267f6c09cb392371fdb17310914d9507a9d318` via preserved merge `6668d0f`.
This initial snapshot is retained; the subsequent review correction is below.

Commands run from the application checkout with the nested manifest:

```sh
cargo fmt --manifest-path qualification/section-2/Cargo.toml -- --check
cargo test --manifest-path qualification/section-2/Cargo.toml --locked --offline --features gui
cargo test --manifest-path qualification/section-2/Cargo.toml --locked --offline --features gui --doc
cargo clippy --manifest-path qualification/section-2/Cargo.toml --locked --offline --features gui --all-targets -- -D warnings
cargo build --manifest-path qualification/section-2/Cargo.toml --locked --offline --features gui --release
SLINT_BACKEND=winit-skia-software qualification/section-2/target/release/qualification-desktop --controller-smoke
SLINT_BACKEND=winit-skia-software qualification/section-2/target/release/qualification-desktop --smoke
SLINT_BACKEND=winit-skia qualification/section-2/target/release/qualification-desktop --smoke
git diff --check
```

Rust/Cargo1.99.1, isolated temporary rustup/Cargo cache. Format, seven tests,
Clippy warnings-denied, separate doctests (zero examples), optimized build,
public-component assertions and three-second backend smoke pass. Final CPU smoke
construction marker78 ms; it is not visible-frame latency or input p95.
Final raw binary SHA-256:
`23a0f23ba0eda4347eace3dc1715ae57cb22751343fa4ea6b1e2301f38901dc6`.

The first sandboxed Clippy run failed because Skia's build script attempted an
official upstream cache download despite Cargo `--offline`; the permitted network
rerun passed. Cargo offline alone is not a hermetic native dependency build.
The release build replays cached rust-objcopy/libLLVM debug-strip warnings from
the initial direct-Cargo environment; it still exits0. The final UI binding cycle
is fixed and final Clippy emits no UI warnings. Neither limitation is hidden as
passing hermetic packaging evidence.

The unsigned DMG in packages.md predates `ea9ad3a` and has its own retained binary
hash. Its mounted-path smoke is historical package feasibility, not a package
qualification of this final head. No package, native AT, IME or clean-host claim
is promoted by these local checks. Existing provider CI results are Go/Wails
lanes, not Rust/four-target qualification. Root CI ownership stays with librairii-5.

All tests are synthetic/temp/read-only source fixtures; there is no production
library/config opening or production endpoint. PR39 golden comparisons await
its reviewed/frozen corpus. No external OpenSpec checklist item is checked.
Physical/host/distribution/selection gates and 2.13 AO review remain pending.

## Required review correction after9eef0a2

The partial-scope native review of9eef0a2 requested one P2 fix: actual checkbox
toggle broke the one-way binding, leaving checked state stale after Apply. The
separate fix is `31ccd9e`; regression.md records the pinned sources, writable
two-way properties and the new public in-process `--widget-smoke` route.

The widget-event regression failed before correction at the actual checked-state
assertion. After correction it passes twice per run in both debug and optimized
builds, without showing an OS window or sending OS input. It drives the CheckBox
and Apply Button handlers through public WindowEvent dispatch instead of directly
calling controller callbacks. Controller smoke, seven GUI-feature Rust tests,
separate doctests, rustfmt and all-target GUI-feature Clippy with warnings denied
also pass after the correction. Optimized build succeeds with the same historical
cached rust-objcopy warning described above. No new dependency/test license is used.

This supersedes the initial code snapshot for the checkbox behavior only. It does
not establish actual native keyboard, accessibility, IME, visual or physical-host
observations. All remaining qualification gates and final-head review stay open.

## Provisional2.10 corpus experiment after558b2b4

Code/evidence commit `a3a74a3da408c572e9e6b3fdb91dc6c4fe53ee56`; exact pinned
PR39 input e7c771b and every selected input hash are in corpus-observations.json.
The same full report was reproduced identically twice. Exit2 records30 measured
differences:24 SQL-only Collection fields and six independent producer/candidate
Unicode filenames. The decoder experiment is not a product-classification pass;
corrected corpus admission and full2.10 remain pending. Details and exact commands
are in data-feasibility.md.

Cumulative local checks on the implementation above:

* cargo-nextest0.9.148, `run --locked --offline --features gui`:7/7 pass,0 skipped.
* The same locked nextest command with `--no-default-features`:7/7 pass,0 skipped.
* Separate GUI-feature doctests:0 examples, harness passes.
* All-target GUI-feature Clippy `-D warnings`, rustfmt check and diff check pass.
* Four Python semantic comparison regressions pass.
* Debug and optimized builds pass; both original public controller smoke and
  actual-widget Space→Apply→unchecked-twice regression pass in each configuration.

This uses the existing temporary nextest executable coordinated with tooling
owner5; no root workspace, toolchain or CI file changed. Initial sandboxed native
build-script downloads failed DNS; permitted network reruns passed using the
official Skia cache. Cargo offline is still not hermetic for that native graph.
The optimized build retains the historical cached rust-objcopy warning above.

Integration origin remains41267f6 and is already merged through6668d0f. Provider
CI at558b2b4 passed its existing Go/Wails lanes; it is not Rust platform evidence.
The addressed inline thread is resolved and partial code approval at558b2b4 is
retained. New2.10 code needs another native review of the pushed final head.
PR38 stays draft/unmerged; host/access, licensing, physical tests, clean-host
packages, corpus admission/resource ceilings and toolkit selection remain open.

## Admitted corpus and required toolchain correction

This snapshot supersedes the provisional admission status above. PR39 final
`1cd22443cae3b548bb7f9b3fb2435dcd73c3c65c` is admitted through integration merge
`9a6c1c116d9df726a061ad6aac3955df4ed41263`, included here by preserved merge
`e28e0e66ebb69dbcea4402d462e146bb52448ff8`. No section1 file was edited locally.

Native review714ecc57 of87d6ecd required one wrapper correction. Dedicated fix
`4a0002ac55243e134bd61c0e206f276be9c8468f` uses the channel read from the nested
rust-toolchain.toml through explicit `rustup run`, with the same working directory
for Cargo execution and Cargo/rustc provenance. A real-command Python regression
sets the caller override to1.97.1 and verifies both pinned tools remain1.99.1.
No fake compiler, global default change or new installation is used for that test.

From the application checkout, the documented root invocation was run with normal
Homebrew rustup proxies on PATH, the isolated Cargo cache, and no RUSTUP_TOOLCHAIN
or RUSTUP_HOME override. Global `rustup show active-toolchain` stayed stable/default;
`rustup run stable rustc --version` reported1.97.1 before and after. The report
records actual Cargo1.99.1 and rustc1.99.1. Python3.11+ uses standard tomllib.

Admitted-input consumer/evidence commit `d3a640c0cf794e3160ca5ff443c76f67e0013250`
verifies all146 manifest file lengths/SHA256 and exports87 selected inputs to temp.
Two full report runs are identical. All19 database before/online-backup snapshots
and20 archive observations remain as recorded;24 SQL-only Collection fields and
six independent Unicode filename discrepancies remain exact and exit2. The
corrected representative database has a new verified byte hash and matching
semantic snapshot. Profiles/relocation evidence is byte-verified, never replayed.

Current-base cumulative validation:7/7 locked/offline nextest with GUI and7/7
without defaults, separate GUI doctests (zero examples), five Python tests,
rustfmt, all-target GUI Clippy warnings-denied, and debug/release controller plus
actual-widget regressions all pass. No assertions were removed. Native review
and provider CI must cover the next pushed final head. Mandatory physical,
host/access, licensing, full codec/classification/resource and toolkit-selection
gates remain open; PR38 stays partial/draft/unmerged.
