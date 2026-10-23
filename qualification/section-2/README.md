# Section 2 qualification spike

**Current continuation (2026-10-23):** Linux/macOS only. See
[continuation.md](continuation.md), [current-pins.json](current-pins.json) and the
[runnable operator checklist](operator-checks.md). The owner chose Slint with
About-screen attribution and can operate Intel/Linux hosts. Toolkit selection
still awaits mandatory physical/AT/input/rendering/performance/clean-host gates.
PR38's **Task 2 native qualification** workflow produces matching probe packages.
macOS bundles now have verified ad-hoc integrity seals, without Developer ID or
notarization; their public API tests do not substitute for operator observations.
The original reports below retain their historical source attribution.

Current public GUI regression command, on an eligible desktop:

```sh
cargo build --locked --features gui --bin qualification-desktop
SLINT_BACKEND=winit-skia-software python3 run-smokes.py target/debug/qualification-desktop
```

On headless Linux, wrap the Python command with `xvfb-run -a`. This supplies X11
in-process GUI regression evidence, not Wayland/Orca/physical qualification.


**Partial qualification; no selected toolkit or supported-platform claim.**
Owner: librairii-4. Base: integration commit
`1419f1d7c5236016c0e330056c9eff45931ddf48`.
Only this directory belongs to section 2; it is an independent Cargo workspace.

The original 2026-10-20 reference manifest was checked byte-for-byte:

* Editable two-concept source: `44cd023b9e09e9bd2b411e76e58f5bef70e18b72508e03f25c8aba6343083460`.
* Export: `4df3afb9a27c780657f3c4bf0f2b691f346bd00e589468516a5bc655230d2cb1`.

Section 1 owns the contract/freeze. These references guide the sidebar/inspector
and separate approved-member/review areas; synthetic controls do not port product
query, membership, recovery or archive policy. The representative review is not
the complete product review surface. No HTML or webview is embedded.

## Run on an eligible desktop

```sh
cd qualification/section-2
cargo test --locked --no-default-features
cargo test --locked --doc --no-default-features
cargo build --locked --features gui
SLINT_BACKEND=winit-skia cargo run --locked --features gui --bin qualification-desktop
SLINT_BACKEND=winit-skia-software cargo run --locked --features gui --bin qualification-desktop
```

`--smoke` closes the window after three seconds. Construction timing is diagnostic
only, not time-to-visible/input p95. The UI creates 1,000 artificial multilingual
titles, virtualized list/three-column rows and one shared original bitmap. This
does not exercise 1,000 distinct decodes or an artwork eviction policy. French
labels, details/settings/About overlays, three densities, a distinct review area,
explicit Apply, and a cancellable owned synthetic worker provide probe surfaces.
Progress has one atomic snapshot and a 30 ms UI poll; shutdown cancels and joins.

There is no data/config root, file import, network client, profile opening or
production endpoint in the desktop spike. Data feasibility tests use only
`tempfile` roots and checked-in synthetic/source fixtures. Native picker/reveal
probes must stay confined to a disposable fixture directory.

## Observed build / execution (2.3)

2026-10-20, librairii-4: Rust 1.99.1, macOS 15.7.7 ARM/M4 Pro, SDK 26.2.
`cargo build --locked --features gui` succeeded. An initial width/min-width
compiler error and layout-binding-cycle warnings were fixed before this commit;
the immediate rebuild emitted neither. A later fresh Clippy configuration exposed
the responsive binding cycle again; conditional desktop panels were removed in a
separate fix, retaining narrow disclosure as an unfinished gate.
`SLINT_BACKEND=winit-skia ... --smoke` exited 0,
reporting model construction 160 ms. Native visual/AT inspection is separate.
Two controller tests pass and doctests contain zero examples.

2.3 remains **partial**: other targets have not built/run; the bounded review
fixture, complete localization, modal semantics/focus, responsive disclosure and
native adapters still need expansion/observations. Overlay presence is not proof
of a modal accessibility tree or focus trap. No full feature UI may start.

See `ADR.md`, `hosts.md`, the per-gate records and `ledger.md` for status/evidence.

The bounded `QUAL2.10-392-COUNT-BUDGET` follow-up is source-level partial work in
[decoder-budget/](decoder-budget/README.md). Exact source/patch identities, safe
checks and host setup failure chronology are in
[count-budget-readiness.json](count-budget-readiness.json). Its new containment
controls and retained two-negative/one-benign decoder experiment are **NOT RUN**:
the shared Docker daemon stopped responding after ENOSPC. No new constructor
refusal, general resource bound or production strategy is inferred.

For explicit headless SQLite/archive comparisons against the pinned admitted PR39
fixtures, see `data-feasibility.md` and `probe-corpus.py`. The comparison retains
24 SQL-only/Go-open differences and exits2. No full-corpus parity is claimed.
Independent codec experiments retain six additional filename differences and a
reproducible original Zstd fixture. `decoder-resources.md` / `decoder-observations.json`
record bounded LinuxARM experiments, including a pre-callback OOM; they do not
implement product admission or qualify the candidate's production resource limits.
