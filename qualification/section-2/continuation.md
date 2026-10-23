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

List/Small covers/Large covers now use distinct original 48×64 rasters for the
1,000 synthetic stories. This is bounded raster/model evidence, not a decoder,
lazy-thumbnail cache or input-performance qualification. A main-thread timer
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
record was created successfully. The owner can execute Intel and Linux checks;
actual operator results, clean-host launch and all three-target package evidence
remain pending. The current workflow supplies matching unsigned artifacts.

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
