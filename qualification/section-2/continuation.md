# Linux/macOS qualification continuation

Task 2 resumes from PR38 on 2026-10-23, with the epic through
`ccee9e8` merged without rewriting the original subtask commits. The active
targets are macOS ARM, real Intel macOS, and Linux x86-64 (X11 and Wayland).
Historical Windows observations are retained; Windows is not a completion gate.

## 2.3 representative native spike

The continuation implements all 300 synthetic review candidates in pages of 25.
Not now remains the default, page changes retain explicit decisions, and Apply
clears every pending checkbox. The first checkbox remains a static widget for
the real widget-event regression; the other 24 use writable model bindings.

List/Small covers/Large covers use distinct original 48×64 synthetic images for
the 1,000 stories. The 2.9 continuation below adds actual PNG decoding and a
bounded lazy cache; physical input and product-image qualification stay open.
A main-thread timer
samples actual logical window width to select compact/intermediate/wide states;
it avoids a cyclic dependency between layout minimums and window width.
Compact navigation has an explicit close/Escape path and returns focus to its
invoker. The inspector remains available through Details at narrower widths.

The spike uses the toolkit palette: the first native observation found unreadable
foregrounds when hard-coded light surfaces met the host's dark appearance.
Settings/details/About now disable underlying actions, focus Close initially,
handle Tab/Shift-Tab/Escape, and restore the invoking control. The accessible
overlay is explicitly a groupbox; actual modal/AT semantics remain a 2.4 gate.

Local ARM observations on this source: locked GUI build without warnings;
three pure controller tests; public component controller smoke; actual checkbox
Space → Apply → reset repeated twice. The widget smoke shows the window so its
scroll pane is laid out; key events remain in-process, not OS keyboard input.
Native observations, other target builds and independent review are separate.

Historical decoder/Unicode reports remain unchanged. The current epic supplies
the qualified archive adapter, recorded hardening exceptions and frozen core
normalization; superseded candidates are not being restarted as product work.

## 2.4 focus and accessibility continuation

Native Computer Use now opens `dev.librairii.qualification` successfully. The
2026-10-23 ARM observation used binary SHA256
`c31f9b5bc013538939033c9bf9299e3612991cdd8738b4f7177f66079e07990f`:
Settings exposed English/French/Close, background actions became disabled, four
native Tabs stayed in Settings, Space activated French, and Escape closed it.
The new palette renders readable text in the observed dark appearance.
This is an AX/keyboard/visual observation, **not VoiceOver speech qualification**.

That observation exposed a close-time race: the background actions could still
be disabled when focus restoration ran. Modal state now changes synchronously
before focus transfer. `--focus-smoke` checks actual initial Close focus, four
Tab events, Escape, restored Settings focus and retained unsaved multilingual
input. The corrected in-process test passes. Live status uses a polite region;
completion/cancellation emit one terminal message, and a synthetic error action
provides an explicit retry announcement probe. Spoken output and all other
required targets remain open until observed by the corresponding operators.

## 2.6 native adapter continuation

The stock Slint macOS menu adds a default application menu without a Settings
role. The probe now uses the public, already resolved muda0.19.3 package directly
on macOS, with defaults disabled. It owns application Settings/Command-comma,
AboutSlint, Services/Hide/Quit and standard Edit actions. Linux retains the Slint
application menu and Control-comma. No private Slint API or source patch is used.
A bounded atomic command set retains/coalesces native Settings/About requests
until the main-thread timer can deliver them; opening a second dialog waits for
the current one to close. `--settings-early` uses that same command sink.

Picker titles and all adapter outcomes now have English/French text. The owned
fixture has two distinct difficult filenames so multiple selection is actually
exercisable. Locked ARM build, focus and widget smokes pass. Exact native menu,
picker, reveal and other target observations remain separate qualification.

## 2.11 unsigned operator packages

`package.py` assembles the actual native binary, manifest/architecture/digests,
resolved dependency license inventory/notices, tagged Slint and native Skia BSD
notices, and operator instructions. It refuses existing destinations and wrong
native architectures. These tarballs are synthetic unsigned spike packages,
not signed production DMGs or a completed native third-party notice audit.
`observe.py` verifies package member bytes/containment, native host and macOS
translation status, then creates only **not_run** observations. Three regression
tests reject modified bytes, outside aliases and an unlisted executable, and
verify that a valid package cannot pre-fill passing qualification.

An ARM development tarball assembled locally and its verified observation
record was created successfully. The owner later clarified that they have no
Intel Mac or Linux desktop; interactive checks on those hosts remain unavailable.
Actual operator results and clean-host launch remain pending. The current
workflow supplies matching unsigned artifacts and three-target automated package
evidence, without closing those interactive checks.

The macOS package also retains notices inside the application Resources and
assembles an unsigned DMG in addition to the tarball. Repeated build profiles
may leave several native Skia notice copies; packaging requires their exact
content digests to agree instead of choosing among conflicting licenses.

## Independent review correction: preserve focus from menu invocations

The first focus smoke covered only the in-app Settings button. Independent
review found that menu/accelerator invocation always restored Search. Main-view
controls now record stable focus bookmarks; closing a menu-invoked dialog asks
the still-mounted matching control to restore focus after enablement. Story and
review identities are stable across localized labels, and the virtualized view
retains its mounted range during the dialog. The focus smoke now runs an actual
UI loop and checks Review checkbox → menu Settings → Escape → same checkbox,
as well as the button route and preserved multilingual draft input.

## Independent review correction: route Quit through owned shutdown

The predefined AppKit Quit action could terminate the process before Rust's
post-event-loop join/cleanup. Quit/Command-Q is now a public native menu command
into the bounded command sink, and Linux Quit/Control-Q uses the same path. The
main thread requests event-loop exit, cancels and joins the actual worker, then
explicitly closes the owned temporary fixture. Quit remains available during a
dialog and does not consume a pending Settings command. `--quit-smoke` starts
real synthetic work, requests Quit only while progress is strictly between zero
and completion, then asserts the worker was joined and the fixture is absent.
Repeated identical menu enablement no longer performs native calls every tick.

Re-review confirmed both P2 corrections and found one P3: the Linux menu group
still disabled Quit during dialogs. Only Settings/About are now disabled, leaving
Quit reachable. No other blocking implementation findings were reported; this
review approves only the implemented continuation, not physical qualification.

## 2.9 bounded PNG and cache probe

The GUI now requests lazy images from an owned worker using the already locked
image0.25.10 PNG decoder (default features disabled). The worker creates original
48×64 PNG bytes, then measures decoding separately from encoding. It does not
accept files or untrusted image bytes. Requests/results have 16 slots each,
in-flight identities are capped at 33, and shared decoded images use a 64-entry
LRU. Deferred visible interest is deduplicated within the fixed 1,000 identities;
capacity return retries it even without another incidental model read. Decode
samples retain at most 512 values. Both owned workers cancel/join before normal
Quit retires the temporary fixture. Mounted delegates can retain their visible
images independently of shared-cache eviction, avoiding immediate reload churn.

`--thumbnail-smoke` decodes all 1,000 identities while synthetic work and public
component key dispatch run, verifies the cache reaches its 64-entry cap, then
exits through owned shutdown. It reports cold first-24 completion, total workload,
PNG decode p95 and component-dispatch p95/sample count. These diagnostics do not
measure physical input, first presentation, representative product artwork or
the retained 24-artwork/full-PNG 50 ms budget. Unit regressions verify LRU recency,
reload, distinct pixels, deferred visible retries, saturated queues and join.
Independent review found the initial missing backpressure retry; the bounded
deferred-interest path and no-incidental-read regression correct it.

## CI correction: coordinate inherited cancellation fixtures

All three native qualification lanes passed at PR revision `0048b5f`. The
required product workspace Intel lane then exposed a pre-existing test-fixture
race: a valid early client timeout/cancellation can precede a connection or full
headers, while the fixture still requires a request and fails ten seconds later.
The unchanged original test passed 20 local repeats; reducing its request timeout
to 1 ns reproduced the exact `fixture request deadline` failure. Immediate
cancellation independently reproduced the related partial-header fixture error.

Only the adapter test fixture changes. A bounded stoppable server now holds the
response until operation completion, signals actual complete-header receipt for
in-flight cancellation, and joins even when no headers arrive. The test retains
the 500 ms completion assertion and 30 ms timeout, adds the before-connect 1 ns
case, and checks pre-cancelled fixture retirement separately. The latter proves
owned teardown, not absence of TCP traffic. Production networking, cancellation,
request limits and deadline policy are unchanged. No retries/skips or relaxed
performance threshold hide the failure.

## 2.11 downloaded macOS bundle failure and repair

The owner downloaded ARM artifact `8147bc4` and reported Finder's “damaged and
can't be opened” message on macOS 15.7.7. All distributed-byte digests matched,
but both `codesign --verify --deep --strict` and Gatekeeper assessment reported
`code has no resources but signature indicates they must be present`. The
linker's standalone ad-hoc Mach-O signature did not seal the final app bundle's
Info.plist and notice resources. Raw-binary GUI smokes could not catch this.

Packaging now ad-hoc signs the fully assembled bundle, verifies its resource
seal, then hashes the signed executable and all signature files. Native
regressions verify that subsequent Info.plist or notice tampering is refused.
The recorder rejects a matching-byte app with an invalid bundle signature. CI
extracts each finished tarball, verifies its bytes/native host, and on macOS
verifies the resource seal and exercises the actual packaged app again. It also
retains Gatekeeper's separate assessment without claiming that ad-hoc signing
supplies Apple developer trust.

An isolated copy reproduced the failure before sealing and passed strict
integrity verification after sealing; Gatekeeper still rejected the ad-hoc
identity. The original downloaded copy and reported failed launch are preserved.
Developer ID/notarization and actual clean-host download admission stay open.
An operator may explicitly approve a valid probe through Apple's ordinary
per-app UI; that is recorded separately from default-policy admission. No
quarantine removal, global security override or approval automation is used.

## 2.5 / 2.6 native Search editing repair

The owner's ARM paste failure on `9f50fc6` was reproduced with native clipboard
input into focused Search. Predefined AppKit editing selectors had no Winit
responder. The adapter now connects custom Edit items/accelerators to public
LineEdit functions, with actual focus and toolkit/native dialog guards. The
native picker scope temporarily retains AppKit's standard Edit roles and restores
the application menu on return/unwind. Native Search paste, Unicode replacement,
Undo/Redo, picker menu-click clipboard roundtrip, unchanged background Search,
and Search paste after picker cancellation were observed. See regression.md for
the original red check and its passing result. Native picker keyboard shortcuts
remain unresolved in automation; physical input/IME/AT qualification stays open.
`--edit-smoke` adds bounded widget/model/undo/ownership regression coverage and
explicitly does not label component callbacks as native clipboard evidence.

Physical follow-up at `af132fe`: Maxime confirmed Command-A replaces the complete
Go-to-Folder path using an actual keyboard. The matching automated failure also
occurs in an authored AppKit-only picker, so it is retained as an automation
limitation rather than an application selection defect. The attempted rfd async
change did not help and was discarded. The exact observation and still-pending
physical gates are in [native-picker-input.json](native-picker-input.json).
Maxime also confirmed physical Unicode paste/cut/restore in Go to Folder and
Search paste after picker cancellation. Both fields pass; the failed automated
picker shortcuts are false failures and do not call for an application patch.
All three native qualification lanes, all required Rust targets and existing CI
pass for `af132fe`; those results do not supply missing physical observations.

## Owner scope update: skip VoiceOver execution

On 2026-10-23 the owner explicitly declined VoiceOver testing. Skip its execution
for this qualification; it no longer blocks the current Task 2 selection/merge
gate. Do not report spoken macOS accessibility as qualified. Continue the
remaining keyboard, Linux, renderer, package and performance checks.

## 2.7 immediate runtime inspection

The qualification workflow now inspects the verified archive-round-trip binary
with native `otool` or ELF/loader tools and retains raw evidence, declared bundle
and binary OS requirements, linked libraries and Linux symbol-version floors.
Local ARM import inspection passes; all 18 immediate imports are OS libraries or
frameworks. The debug executable's Mach-O minimum is 11.0 while its verified app
bundle requires 15.0; both are recorded without claiming older-OS support.
Regressions cover substituted/aliased binaries, numeric ABI ordering, unresolved
dependencies, loader paths, minimum-OS parsing and retained timeout output.
All three release packages subsequently passed immediate link inspection in
[CI run 39950896230](https://github.com/01max/librairii/actions/runs/39950896230)
at PR head `9dc3fca`; the exact runtime requirements are in
[renderers.md](renderers.md). Dynamic desktop dependencies and clean-host
qualification remain separate open gates.

## 2.7 forced GPU initialization failure

The new Linux probe runs the verified packaged executable in a separate Xvfb
session. It compares a working OpenGL control with GPU-only failure controls and
the normal renderer's automatic recovery under scoped EGL/Vulkan discovery
failure and disabled GLX. A passing result requires actual backend diagnostics,
French/CJK snapshots, changed scene pixels and Search widget/model assertions.
It retains source/binary/frame hashes and raw failures. The public snapshot API
re-renders into CPU pixels; separate performance diagnostics identify the active
surface. This adds no driver modification or application fallback policy.
Linux execution/visual review are pending CI; physical input, IME, Wayland,
remote desktops and post-start GPU loss are outside this automated result.

## 2.9 startup, idle CPU, peak RSS and distribution measurements

The resource continuation adds a ten-second quiet phase followed by the shared
1,000-PNG/component-input/background workload and normal owned shutdown. The
parent measures launch-to-first-UI-timer readiness, sustained cumulative process
CPU and RSS, plus native full-run peak RSS. It verifies the measured package and
archive identity, retains distribution byte counts/hashes and records actual
surfaces, hardware, scale and session. One warmup plus three fresh-process runs
per normal/CPU request produce raw observations and min/median/max diagnostics.
Missing markers, counters, coverage or successful teardown fail the run rather
than inventing measurements. See [performance.md](performance.md) for units,
observer overhead, phase boundaries and exact limits. CI execution is pending;
physical input, presentation, representative product-artwork budgets and new
evidence-based ceilings are still separate unfinished parts of 2.9.
