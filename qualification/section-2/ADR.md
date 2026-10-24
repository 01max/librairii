# Candidate toolkit and dependency qualification (task 2.1)

Current 2026-10-23 decision: see [current-pins.json](current-pins.json) and
[continuation.md](continuation.md). The owner explicitly chose Slint with
About-screen attribution under the pinned royalty-free desktop route. This
selects the candidate licensing route, without changing Librairii’s own license
or declaring the toolkit qualified. Ordinary public component/event APIs remain
the test route; no commercial test tooling is enabled. The tagged terms are
retained verbatim in [notices/](notices/Slint-Royalty-free-2.0.md).

The2.5 composition observer additionally enables Slint's versioned public
`unstable-winit-030` feature. It records delivered IME events, always returns
`Propagate`, and never creates composition events. This feature is explicitly
unstable across minor releases: the exact1.18.1/0.30.13 pins and hook behavior
require requalification on upgrade. No added production input mechanism or
paid automation entitlement follows.

The record below is historical research. The production data graph has since
been implemented in merged Tasks 4/5/7: bundled SQLite, rawzip/flate2 and the
reviewed pinned sevenz-turbo derivative, plus the frozen core query codec.
The rejected sevenz-rust2/Unicode experiments are not production dependencies.
Windows is deferred; current qualification covers three Linux/macOS targets.

The Linux accessibility graph includes one local backport of
`accesskit_atspi_common=0.19.1`. Its disabled-state translation incorrectly
advertised disabled buttons as enabled/sensitive in retained native modal
evidence. The real NodeWrapper regression fails on the original code and passes
with upstream correction `6ee0558b6315b3ef1594db24ce45a030ecac7cb5`. The selected
candidate keeps every dependency version/feature, records original registry and
source hashes, and preserves MIT/Apache/Chromium texts plus a modification notice.
The workspace runs the adapter's seven existing tests and two new regressions.
Fresh native modal-state evidence is required; this repair alone does not
complete accessibility or toolkit selection.

Date: 2026-10-20. Owner: librairii-4. Status: **proposed candidates; toolkit not selected**.
Scope: P02–P07, P13, P18–P19, P27–P38, P53, P56–P59.

This standalone experiment does not establish the application workspace or port
domain policy. Section 1's reconciled oracle, limits, and fixture freeze remain a
prerequisite for behavioral parity. Full feature UI is gated on 2.13.

## Candidate graph

`candidate-releases.json` records exact releases, publication dates, upstream
repositories, registry checksums, available features and primary API URLs.
These are direct candidate pins, not an assertion about a resolved lockfile.

The nested spike now resolves Slint1.18.1, Winit0.30.13, skia-safe0.153.3,
AccessKit0.24.1/accesskit_winit0.33.2 and Slint's own muda0.19.3. The separately
considered muda0.21.2 is not added directly. Rust1.99.1 is installed only in this
session's temporary rustup/cache; no project-root pin or global toolchain changed.
`Cargo.lock` fixes this experimental graph; it does not select the product graph.
Candidate pins were sent to root-workspace owner librairii-5.

| Component | Candidate | Explicit intent / terms |
| --- | --- | --- |
| Rust / Cargo / rustfmt / Clippy | 1.99.1 | Stable 2024 edition; isolated toolchain, no global update. [Release](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/) |
| Slint and slint-build | 1.18.1 | MSRV 1.92. Disable default backends. Compile std, compat-1-18, accessibility, Winit X11/Wayland and Skia; build embeds resources. [Release](https://github.com/slint-ui/slint/releases/tag/v1.18.1) |
| Winit / Skia wrapper | 0.30.13 / skia-safe 0.153.3 | Match Slint 1.18.1 dependency ranges, resolve in spike lockfile; Apache-2.0 / MIT wrappers. Skia itself is BSD-3-Clause with additional bundled third-party notices. [Slint renderer source](https://github.com/slint-ui/slint/blob/v1.18.1/internal/renderers/skia/Cargo.toml), [Skia notices](https://skia.googlesource.com/skia/+/main/LICENSE) |
| rusqlite / libsqlite3-sys | 0.40.2 / 0.38.2 | MIT wrappers; bundled C SQLite plus backup, no extension loading. Record actual SQLite version and compile options at execution. SQLite is public domain. [Features](https://docs.rs/rusqlite/0.40.2/rusqlite/), [SQLite terms](https://sqlite.org/copyright.html) |
| zip | 9.0.5 | MIT; disable defaults/AES, initially deflate-flate2-zlib-rs. Codec expansion requires frozen oracle evidence. No extraction helper. [Source](https://github.com/zip-rs/zip2) |
| sevenz-rust2 | 0.24.1 | Apache-2.0; disable defaults/AES/util/compression. Enable decoder codecs only after corpus inventory. Explicit single-thread limit. [Published metadata](https://crates.io/api/v1/crates/sevenz-rust2/0.24.1) |
| rfd | 0.17.2 | MIT; native macOS/Windows dialogs; Linux portal + Wayland, no GTK default. Parenting/cancellation need observation. [Source](https://github.com/PolyMeilex/rfd) |
| muda | 0.21.2 | MIT OR Apache-2.0; considered for native menus only. Default GTK adds Linux libraries/event-loop integration, so not assumed interoperable with Winit. First test Slint public menu API. [Source](https://github.com/tauri-apps/muda) |
| tempfile | 3.27.0 | MIT OR Apache-2.0, synthetic file isolation only. |

Slint renderer candidates: Skia Metal on Apple, D3D on Windows, GL/Vulkan on
Linux, **Skia CPU** using `winit-skia-software` for multilingual fallback.
`winit-software` is a different renderer and does not satisfy this requirement.
Documented configuration is not observed GPU-failure recovery.
[Backend documentation](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/).
Skia has C++ build/cache artifacts; SQLite has C. Record linker/runtime imports
and upstream binary-cache provenance before redistribution. Fonts are host fonts;
the spike embeds only original synthetic artwork, no downloaded catalog assets.

## Distribution and test terms

The repository has no tracked root LICENSE at the baseline. Product distribution
terms require an owner decision before final selection. Proposed Slint route:
`LicenseRef-Slint-Royalty-free-2.0`, with the actual `AboutSlint` widget reachable
from a top-level About command. Preserve notices; do not expose Slint APIs as an
SDK. GPL-3.0-only and paid Software-3.0 are alternatives, not implicitly selected.
Read the [tagged license](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md).
This candidate record does not change the repository license.

Slint's paid plans may include its GUI test automation framework; an upstream
internal test driver or `system-testing` feature is not assumed freely usable
public application tooling. Use ordinary Rust controller tests and public
component APIs first; qualify OS accessibility automation separately. No paid
tool, license acceptance or purchase is authorized by this experiment.
[Plan terms](https://slint.dev/terms-and-conditions),
[upstream testing infrastructure](https://github.com/slint-ui/slint/blob/v1.18.1/docs/testing.md).

Helper candidates (not installed merely to populate a list): cargo-nextest
0.9.148, cargo-deny 0.20.2, cargo-audit 0.22.2, cargo-llvm-cov 0.9.1,
cargo-fuzz 0.13.2, proptest 1.12.0, insta 1.49.0, Criterion 0.8.2.
Registry records identify source and license: all MIT OR Apache-2.0 except insta
(Apache-2.0). Keep nextest doctests separate; fuzzing needs a separately approved
exact nightly, not the shipping compiler. Section 3 owns application tooling.

## Alternatives and decision rule

Fallback candidate eframe/egui_kittest 0.37.0 (MIT OR Apache-2.0, MSRV 1.95)
must face the identical gates if Slint demonstrably fails one. Inaccessible
hardware is missing evidence, not proof the toolkit failed. Iced accessibility
and Qt/CXX-Qt deployment remain comparison concerns documented in the change's
research; no parallel feature implementation is justified yet. Native means
directly rendered, not all OS widgets. No webview fallback is permitted.

Slint release/security maintenance and decoder releases are active upstream, but
that does not promise long-term ABI/API or corpus compatibility. Refresh candidate
metadata deliberately; rerun affected locked builds, corpus and platform gates.

## Implementation order

1. Record host/access/SDK/runner inventory and exact missing resources (2.2).
2. Build isolated synthetic UI and data probes (2.3, 2.10); freeze reference digests
   with section 1 before interpreting behavior as parity.
3. Execute and attribute each physical gate (2.4–2.9, 2.11); retain missing cases.
4. Trigger fallback only for an established mandatory failure (2.12).
5. Leave 2.13 pending AO native review and all mandatory physical results.

## Unicode15 compatibility qualification follow-up

The independently owned [Unicode experiment](unicode-compat/README.md) evaluates
unicode-normalization=0.1.22 (defaults off,std) and
unicode-general-category=0.6.0 (defaults off), with its own locked graph and
upstream source/notices. Actual Rust1.99.1 ARM execution against Go1.26.5 in a
verified disposable oracle clone covers all1,112,064 valid scalars and15,550
source-derived/admitted text sequences. Raw NFKD retains570 CGJ differences;
stream-safe NFKD and final pipelines match that tested set. These are candidate
observations, not a production graph choice, all-string proof or2.1 completion.

Subsequent bounded supplements retain896 raw/672 stream-safe intermediate
differences in896 mixed-class cases, then160 raw/120 stream-safe and120 differences
in **each final pipeline** among160 surviving-Mc cases. Final parity of both
tested crate compositions is **REJECTED**. CGJ insertion before reordering leaves
observable surviving-mark order after Mn removal. Earlier reports remain
byte-identical and scoped to their measured commits; the regression retains the
actual Go case0 witness. No compatibility exception or replacement golden is
introduced, and no new composition/production graph is selected.

The exactDefault CLDR32 package/data/source closure is read-only and versioned;
there is no qualified Rust language canonicalizer here. Root's accepted private
compatibility/frozen-Go translation route still needs its own implementation,
source-policy/notice decision and validation under owners5/6. Existing data,
decoder, physical, product license and toolkit gates are unchanged.
