# Partial spike validation snapshot

2026-10-20, observer librairii-4. ARM host/SDK inventory: hosts.md.
Code tested at `954a951` (UI fix `ea9ad3a`); includes integration
`41267f6c09cb392371fdb17310914d9507a9d318` via preserved merge `6668d0f`.
Later documentation-only ledger commits do not change this tested code.

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
