# Task 14 independent review

Fixed comparison base: `08cdae9fc9bd690d2927731252f40b5bc48953e5`.
Independent read-only Standards and Spec reviewers examined the section commits
and corrections through `03c6d38`. This review closes the presentation-foundation
boundary before Task 15 composes complete workflow views.

## Standards

Final source acceptance: **0 required findings**.

The two original findings were corrected: the activation namespace now recognizes
and regular-file-validates both native UI preference documents, and the native
fixture resolves the actual executable from Cargo's compiler-artifact JSON.
The verifier retains, executes and hashes those exact bytes, with clean-source
and before/after executable checks.

No additional actionable documented-standard violations or baseline smells were
found. The generated lint exemption contains only compiled toolkit output.
The original 596 required test identities retain their exact prefix and order;
twelve additions match the source-map registrations. Historical fields and source
pins remain unchanged, and the manifest/checker updates only repin the current
map hash.

## Spec

Final source/local-evidence acceptance: **0 remaining findings**.

Four source findings were corrected: dynamic sidebar group invokers regain focus
after dialogs; the original AboutSlint attribution has a localized keyboard
website action; only system tag labels/values translate; and manual Shelf
queries retain membership order without changing the durable sort preference.
Real-library and catalog regressions cover the latter two behaviors.

The additional rendered compact-layout finding is corrected. Actual 560×400
captures show full story rows, external captions/warnings, paging and Details.
Workspace shows a complete approved member, and keyboard focus reveals a complete
review choice. The fixture asserts usable viewport bounds as well as dimensions.
No scope creep was identified. Complete workflow transactions remain Task 15.

## Evidence boundary

The independently audited local ARM receipt identifies `0ff9d7b` and contains
74 sealed payloads, including the exact retained executable. Each observed scale
(1 and 2) contains 24 concept/language/density/mode frames. Minimum render geometry
is 560×400 / 1120×800 physical pixels respectively; browser/approved/review viewports
are 235/90/117 logical pixels. All hashes, executable correspondence, dimensions,
model assertions and keyboard checks passed independent audit.

The complete local Rust gate at `03c6d38` passes **593 tests**, zero skips or
failures, plus formatting, lint, discovery, dependency/source inventory and docs.
The retained Go/frontend `make check` passes. Platform evidence is supplied
separately by the source-labelled artifacts and required checks on
[PR #65](https://github.com/01max/librairii/pull/65), including Linux AT-SPI, native
storage handover and relocation rehearsals. Local component review does not
claim physical HID, VoiceOver, full workflow, resource-budget or package acceptance.

Standards: 0 remaining findings. Spec: 0 remaining findings.

## Hosted Mac capture correction

The first current-source ARM job passed all 593 cumulative tests but correctly
failed the exact screenshot geometry gate: AppKit constrained requested wide
760-pixel content to 653/656 pixels on the hosted display. Failed artifact
[12686898244](https://github.com/01max/librairii/actions/runs/40395097742/artifacts/12686898244)
has ZIP SHA256 `ecccd32c3bc1df1d5a701aaaac9fded2b5cc7cabdf25e15fa8fe1e4cb229988c`.
It is retained as failed evidence, not substituted for a passing receipt.

The content fixture now disables pointer resizing before programmatic macOS size
requests and records observed native resizability/window/monitor geometry. An
isolated local AppKit probe reproduced the constraint with a resizable window
and retained the exact oversized request after disabling resizing. The Spec
review accepts this fixture scope: actual full Skia content may extend beyond
the physical CI display, so the capture does not claim compositor visibility,
physical input or production live-resize coverage. Exact content dimensions and
all existing geometry/keyboard assertions remain required; fresh three-host
execution supplies the platform acceptance.

At the next head, ARM and Linux passed the complete native gate. Intel passed
the 593-test cumulative gate but its capture timer reached the first resize
before Winit had created the native window (zero captured frames). Failed
[artifact 12689507765](https://github.com/01max/librairii/actions/runs/40398767983/artifacts/12689507765),
ZIP SHA256 `b9c0ad4d82a27877267e1b03e0b24a85949fa43d983fb32eea8d2ca7e458299c`,
retains that observation. The timer now waits for `has_winit_window()` before
advancing any fixture step; the existing parent deadline remains the hard limit.
All required captures and assertions still run once the window exists.

## Production macOS menu composition correction

A final Spec review found that the application's Muda bar/handler competed with
Slint's own MenuBar and text context menu initialization. The earlier component
fixture did not install the production menus, so its successful captures did
not establish application-menu ownership or delivery.

The production entry point and native fixture now share one process policy and
window/menu factory. Before threads or toolkit initialization, same-PID `exec`
sets Slint's documented non-Muda context-menu fallback without unsafe global
environment mutation. The qualified Winit builder disables its default bar,
and the macOS Slint component omits its toolkit MenuBar. One application owner
retains localized App/Edit/Navigation actions and standard application commands.
A bounded ordered native-command FIFO preserves repeated history actions; Quit
has an independent atomic path.

Independent Standards and Spec source review found zero required findings in
this correction. The regression now inspects actual `NSApplication.mainMenu`
identity/hierarchy and sends AppKit key equivalents/actions through the real
handler for Settings in both languages, focused Select All/Undo/Redo, ordered
Back/Back/Forward, disabled history, AboutSlint and pending-work Quit. Its report
requires those assertions before publishing successful composition evidence.
Actual local execution also exposed the locked Muda lower-case/Shift Redo
encoding: a canonical uppercase AppKit key equivalent now accepts shifted events
and restores both editor replacement steps. The synthetic event retains Shift
in its characters, as AppKit requires, and asserts actual text effects.
The final label review also corrected Muda's hardcoded Hide/Hide Others/Show All
defaults. Retained predefined items now follow English/French selection, with
actual AppKit title assertions; system-populated Services contents retain OS
presentation.
Fresh source-labelled execution supplies acceptance of this composition; older
component-only receipts remain historical evidence.
