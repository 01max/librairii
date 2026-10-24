# Task 2 toolkit selection

2026-10-24: **Task 2 is complete. Slint is selected for the scoped Linux/macOS rewrite.**

Implementation `e9ab1bad1b5ef9797877f1aa3a8cf577bbc6ffda` passes [native run 70](https://github.com/01max/librairii/actions/runs/40127878713), including ARM macOS, real
Intel macOS and Linux x86-64 X11/Wayland. The [exact matrix](completion-evidence/final-matrix.json)
and [independent review](completion-evidence/review.md) verify package/archive bytes,
source and binary identities, notices, raw observations, retained budgets and cleanup.
All thirteen Task 2 subtasks are complete or conditionally handled. Each subtask
and code review fix retains separate commits on PR38 targeting
`rewrite-in-rust-native-desktop`.

The selected graph is Slint 1.18.1, Winit 0.30.13, Skia 0.153.3 and Rust 1.99.1,
with the pinned AccessKit graph and independently tested upstream disabled-state
backport. The owner-approved royalty-free desktop route uses actual top-level
AboutSlint attribution and leaves Librairii's own license unchanged. Public
component/controller, Winit and OS-accessibility APIs provide the regression route.
The conditional egui fallback is not triggered by the final passing scoped gates.

Qualification covers native pickers/menus/parenting/Settings/clipboard/reveal,
French/Latin dead keys and editing, expanded Orca workflows, native window sizes,
contrast/reduced motion, actual ARM Retina and Linux mixed-scale transitions.
All valid UTF-8 data and filenames remain unrestricted. Windows and script-specific
Japanese/Chinese/Arabic input/IME/shaping are deferred; VoiceOver execution is
excluded by the owner. No physical-HID/first-pixel, human intelligibility or
full-product accessibility claim is made.

All 64-event input and 20-trial navigation distributions retain their 100 ms / 3000 ms
p95 limits. Artwork method v3 reuses compressed/output scratch while preserving
24 actual reads/full CRC-checked 320×400 PNG decodes, every pixel, one warmup and
all 100 measured wall/CPU samples with the unchanged 50 ms wall-p95 ceiling.
All startup-marker/idle/RSS/archive ceilings pass. The offline Linux KVM route
proves reboot/removal/reinstallation and byte-canary preservation; both Mac targets
prove isolated normal/CPU process launch/relaunch with actual denial/positive controls.
These Mac checks do not claim erased OS hosts, minimum 15.0 execution, Gatekeeper
trust, public signing/notarization or product installer/upgrade completion. Task 18
owns the product release lifecycle.

Reviewed bundled SQLite feasibility plus merged Task 5 archive and Task 7 query
qualification supply the data graph. Task 4's storage/profile port remains pending.

The [approved run67 matrix](completion-evidence/run67-matrix.json) and
[review](completion-evidence/run67-review.md) are preserved byte-for-byte.
Run68's Intel artwork failure remains failed: wall p95 61.131010 ms and CPU
p95 51.246 ms. [All paired samples and local diagnostics](completion-evidence/artwork-scratch/)
remain attributed; overlapping local timings do not establish its cause or a
robust speedup. Run66's intermittent Wayland dispatch stall also has an unknown
cause. Fresh run70 passes the unchanged gates without dropping/replacing samples.
Earlier experiments remain in [selection-gate-history.md](selection-gate-history.md)
and [ledger.md](ledger.md); their original verdicts are retained.
