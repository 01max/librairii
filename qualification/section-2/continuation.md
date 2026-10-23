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
