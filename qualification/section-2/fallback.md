# Conditional fallback (2.12)

Task2.12 is handled: the selected Slint candidate passes the scoped mandatory gates
with reviewed repairs. The fallback condition is not triggered; no egui experiment
is required or claimed. See [selection-gate.md](selection-gate.md).

Earlier observations below retain their dates, source commits and historical scope.

Status: **not triggered; toolkit selection remains open**.
No mandatory Slint failure has yet been established by a complete representative
test. Missing Intel/Linux/Windows resources, denied ARM interaction and unfinished
spike modal/responsive probes are missing evidence, not proof Slint cannot meet
the requirement. Local Slint source/layout errors are experiment defects to fix.

If a reproducible mandatory failure remains after a bounded fix attempt, retain
the exact failing package/fixture/host/reader/IME/renderer result and qualify
eframe + egui_kittest **0.37.0** (MIT OR Apache-2.0; MSRV1.95) against the same
mockup digests,1,000-story distribution and all tasks2.3–2.11. Candidate metadata
is in candidate-releases.json. [eframe source](https://github.com/emilk/egui/tree/0.37.0/crates/eframe),
[public test harness](https://docs.rs/egui_kittest/0.37.0/egui_kittest/).

The fallback must preserve independent core types and prove real Intel, all
screen readers/input, multilingual GPU-failure CPU rendering, native integrations,
licensing/tooling and clean-host packages. Do not copy Slint's passing subcases
onto eframe. If neither qualifies, reopen toolkit selection with the owner and
record the failure; never waive accessibility/platform gates or add a webview.
No fallback feature UI or silent dependency switch is implemented here.
