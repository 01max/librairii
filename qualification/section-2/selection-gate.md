# Task 2 toolkit selection

2026-10-24: **Slint is selected for the scoped Linux/macOS native rewrite. Task 2 is complete.**
The final implementation matrix and independent review pass. Detailed artifact/source
identities and gate results are in [completion-evidence/final-matrix.json](completion-evidence/final-matrix.json)
and [completion-evidence/review.md](completion-evidence/review.md).

Implementation `d145d604af917618ac6127bba8687806c5ed032a`: [native qualification run 67](https://github.com/01max/librairii/actions/runs/40123986713).
Historical experiments remain in [selection-gate-history.md](selection-gate-history.md) and
[ledger.md](ledger.md); they do not override the attributed completion evidence.

The selected GUI candidate is Slint 1.18.1, Winit 0.30.13 and Skia 0.153.3 on Rust 1.99.1,
with the pinned AccessKit graph and the tested upstream disabled-state backport.
The owner approved the royalty-free desktop route with the top-level AboutSlint
attribution. Librairii's own license is unchanged. Tests use public component/controller,
Winit and OS accessibility APIs; no commercial GUI test tool is required.

The qualification scope covers ARM macOS, native Intel macOS and Linux x86-64
X11/Wayland. UTF-8 data remains unrestricted; French/Latin editing is the current
input target. Windows and Japanese/Chinese/Arabic-specific IME/shaping are deferred.
VoiceOver execution is excluded by the owner's decision, without a spoken macOS
accessibility claim.

| Gate | Qualified evidence |
| --- | --- |
| 2.1–2.3 | Exact pins/features/notices, actual native builders/SDK access, representative responsive localized 1,000-story spike |
| 2.4 | Independently approved native Orca semantics, keyboard focus, offscreen selection, review paging/Apply and real progress/error/retry speech/audio |
| 2.5 | French dead keys, full selection/Delete, composed/decomposed caret/Undo/Redo, long clipboard text, actual native resizing/contrast/reduced motion; ARM Retina and Linux mixed-scale movement |
| 2.6–2.8 | Native picker/menu/clipboard/reveal cases, forced GPU-init failure/CPU recovery, supported public regression route |
| 2.9 | 64 actual native input samples, 20 expansion/navigation samples under thumbnail/background load, 100 read/full-decode batches of 24 artwork files and measured additional resource ceilings |
| 2.10 | Reviewed bundled SQLite feasibility plus merged Task 5 rawzip/flate2/sevenz-turbo and Task 7 query graph; accepted/rejected corpus retained; Task 4 storage/profile work remains pending |
| 2.11 | Matching sealed minimal packages; fresh offline Linux KVM launch/reboot/reinstall and Mac process-isolated normal/CPU launch/relaunch without development access |
| 2.12 | Conditional fallback is not triggered when the selected candidate passes the scoped mandatory gates after repairs |
| 2.13 | Independent review of exact package/locked/native notices, raw observations and three-target coverage, followed by the recorded decision |

Raw package/probe records retain `toolkit_selected:false`: individual probes do not
declare overall selection. The independent decision here covers the combined scope.
Their hashes and failed outcomes are
preserved; this decision never edits historical evidence into a pass.

The Mac CI AX checks use the host's available content sizes. Wide/Retina frames
are independently retained from the actual ARM development window; Intel's larger
run63 screen also passes all three layouts. Linux must move the same window through
2×/1×/2×/1× while retaining its 1280 × 800 logical size and 1,000-story model. These are
actual geometry/rendering and OS-event observations, not a physical-HID/first-pixel
or universal accessibility claim.

Minimal spike runtime qualification does not claim erased macOS installation,
minimum 15.0 execution, public Gatekeeper trust, signing/notarization or the finished
product installer/data-upgrade lifecycle. Those release concerns remain with Task 18.
The identified native macOS execution is 15.7.9; Linux is Ubuntu 24.04 / glibc 2.39.
The headless merged backend budgets remain unchanged; this task does not duplicate
future feature UI or depend on Tasks 14/16.

Linux display traversal waits for each actual focus change before the next Tab.
Input method v3 checks foreground ownership before timing, then includes the full
injector process and actual edited-model acknowledgement. It retains all 64 wall
samples and the unchanged 100 ms p95 limit, with separate foreground/posting/wait
diagnostics. Earlier failed observations remain failed in the evidence history.

The native notice audit covers the exact selected Linux/macOS graph and conservative
11-component Skia supplement, including Khronos SPIR-V/GLSL header terms. The bundled
MIT/Apache/Chromium AccessKit notices and upstream backport provenance are preserved.
Host fonts are not redistributed. Future graph/platform changes require a new audit.
