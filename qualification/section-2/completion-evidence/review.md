# Independent Task 2 completion review

Status: **approved, 2026-10-24; no remaining actionable findings.**

Reviewer: `task2_continuation_review`, independently of implementation. The review
covers Task 2 under the owner-approved Linux/macOS, French/Latin input scope.
Windows and script-specific Japanese/Chinese/Arabic input are deferred; VoiceOver
execution is excluded. Valid UTF-8 data and filenames remain unrestricted.

Implementation head: `e9ab1bad1b5ef9797877f1aa3a8cf577bbc6ffda`.
All three packages identify clean built source
`cf1208919018f881e2b9769d3d6563eb8516481d` and GUI lock SHA256
`613fcd9b99a9debd0fe125eea159fd63c3e075d77bb322b8b001ec7bb41bd2a3`.
[Native run 70](https://github.com/01max/librairii/actions/runs/40127878713),
[Rust quality](https://github.com/01max/librairii/actions/runs/40127878625) and
[existing CI](https://github.com/01max/librairii/actions/runs/40127878646) pass on
that exact head. Artifact, binary, raw-report identities and all retained artwork
samples are in [final-matrix.json](final-matrix.json).

| Native target | Artifact | Package payloads / tar regular files | Raw hashes | Artwork p95 |
| --- | --- | --- | --- | --- |
| ARM macOS | 12574455518 | 1301 / 1302 | 435 | 17.063041 ms |
| Intel macOS | 12574070368 | 1301 / 1302 | 447 | 16.792325 ms |
| Linux x86-64 | 12574486375 | 729 / 730 | 692 | 11.907650 ms |

Review verifies exact manifest payloads, archive membership and bytes, clean
source/binary/report bindings, packaged notices and every recorded raw hash.
The conservative eleven-component native Skia supplement includes the exact
SPIR-V/GLSL header terms. AccessKit retains MIT/Apache/Chromium notices and the
tested upstream disabled-state backport `6ee0558b6315b3ef1594db24ce45a030ecac7cb5`.
Data qualification combines reviewed bundled SQLite feasibility with the merged
Task 5 archive and Task 7 query graph; Task 4's storage/profile port remains
pending. Retired decoder and normalization experiments remain historical data.

The native matrix verifies all eight adapter cases on each Mac and each Linux
desktop, exact UTF-8 selections/cancellation/outside-root rejection, native
parenting, Settings/early commands, clipboard and observed Finder/Thunar paths.
French dead keys, composed/decomposed editing, Undo/Redo, long clipboard text,
source restoration, ownership and normal retirement pass. All actual input and
navigation distributions and endpoints are recomputed without sample replacement.

Artwork method v3 reuses compressed/output scratch with a fresh full CRC-checked
decoder for every actual file read; packaged metadata agrees with the trace.
Every decoded byte matches the previous DynamicImage route in all 24 fixture
controls. CRC corruption, truncation, wrong dimensions/color and short output
reject. Each target retains one warmup plus 100 complete batches of the same 24
frozen
320×400 PNG file reads/full decodes. All wall and CPU samples, exact pixel witness
and fixture retirement verify; CPU diagnostics never substitute for wall time.
All 24 resource launches (six warmups and eighteen measurements) recompute to
passing verdicts under the unchanged startup/idle/RSS/archive ceilings.

Expanded Orca X11/Wayland evidence verifies fourteen completed utterance/audio
stages per session, actual modal/virtualized selection/review/progress/error/retry
semantics, startup-excluded PCM/WAV windows and owned cleanup. Native display
checks cover fitting Mac sizes, all Linux layouts, contrast, localized modal and
compact Review focus, stable idle frames and actual 2×/1×/2×/1× window movement.
The independently reviewed ARM Retina record in [retina/](retina/) supplies three
actual 2× sizes. Its retained Slint UI and native-probe source files are unchanged from the final implementation.

The offline Linux KVM guest verifies no network/development tools, native desktop
execution, reboot, removal/reinstallation, canary preservation and owned cleanup.
Both Mac targets verify four isolated normal/CPU launches and relaunches with
actual source/toolchain/network denial controls and positive controls.

Failed observations remain failed: run 64 Intel artwork/picker setup, run 65 X11
input/display traversal, run 66 Wayland input and run 68 Intel artwork. The complete
run 68 paired trace remains failed (wall p95 61.131010 ms, CPU p95 51.246 ms);
its mixed CPU/wall cause is unknown. Local overlapping old/new timings do not
establish a repair or robust speedup. The approved run 67 matrix/review are
preserved byte-for-byte as run67-matrix.json and run67-review.md.
The focused native path editing
and acknowledged Tab traversal repairs have fresh passing native evidence. Input
method v3 aligns the foreground-preflight timing boundary across platforms while
retaining the full injector process and actual model acknowledgement, all 64
samples and the 100 ms p95 ceiling. The cause of run 66's intermittent dispatch
stall remains unknown; the fresh passing matrix does not prove it repaired.

This qualifies the pinned synthetic feasibility slice and selected distribution
graph. It does not claim physical-HID/first-pixel latency, human intelligibility,
full-product accessibility, minimum macOS 15.0 execution, erased macOS hosts,
Gatekeeper trust, public signing/notarization or finished installers/upgrades.
Product release lifecycle remains Task 18. The owner-approved AboutSlint route
leaves Librairii's own license unchanged. The egui fallback condition is not
triggered by the candidate's final passing scoped gates.
