# Automated native Linux integration (2.6, partial)

`check-native-linux.py` runs the byte-verified package and matching archive in
separate owned X11/Openbox and Wayland/Sway desktops on native Linux x86-64.
The observer image uses the same immutable Ubuntu 24.04 amd64 base as the
clean-runtime check, with actual package versions retained. Its Python/AT-SPI,
portal and file-manager tools are intentional; it makes no clean-runtime claim.

Each container runs as an ordinary mapped account, offline with a read-only
root, no capabilities and only the package/evidence directories mounted. A
private temporary runtime contains desktop sockets, settings and synthetic
fixtures. Container/image retirement is required. Captured output is bounded to
4 MiB per pipe (overflow fails), graphics buffers live on the capped temporary
filesystem, each desktop run to ten minutes, and each app observation to 45 seconds.
No user's library or desktop session is opened.

The driver observes actual GTK portal dialogs for multiple Unicode archive
selection, single selection, cancellation, folder selection/cancellation and
rejection of a folder outside the app's fixture. It records the actual returned
paths. X11 parent proof compares the dialog's WM_TRANSIENT_FOR and parent PID.
Wayland proof ties the real public portal request to the app's exported handle
and the GTK backend's imported handle/parent association, rejecting invalidated
handles. Sway provides xdg-foreign, absent from the existing Weston 13 session.

After each dialog closes, the driver requires OS focus restoration without
forcing focus, invokes Settings through the OS accelerator, dismisses it and
types into Search through the desktop input protocol. Early Settings delivery
has its own case. Reveal requires an actual visible Thunar window whose native
location field contains the fixture path. A successful command exit alone does
not pass reveal. Each app must join its workers and remove its fixture.

`native-linux.json`, per-session/per-case records, raw portal/protocol logs,
package/image hashes and screenshots are retained in the native qualification
artifact. Every required check and all eight cases must pass in both sessions.
The independent clean-runtime check runs separately; either failure fails CI.

These are synthetic OS events and native AT-SPI actions in headless desktops.
They do not qualify physical keyboards, IME composition, Orca speech, arbitrary
distributions, real GPU hardware or macOS integration. Task 2.6 and the toolkit
selection remain open until their remaining scope has evidence. Native execution at `230cc47` passes all eight cases on each session in
[run40018196257](https://github.com/01max/librairii/actions/runs/40018196257).
[Retained observations](native-linux-observations.json) bind source, binary,
image, archive, raw report, session-file hashes and per-case results. The archive
bytes and contained executable/manifest/source were independently rehashed.
Independent review verifies all 16 cases, 190 session-file hashes, exact portal
responses, X11 parent properties, six Wayland associations through normal teardown
and all 711 archive payload hashes. Both Thunar screenshots visibly show the
actual fixture path. Picker selected/outside-folder images precede GTK repaint
and can retain the old highlight: they prove native-dialog/filename presence,
not final selection. Native row observations and exact callback paths/response
codes provide the selection proof.
The same run preserves a separate Intel timeout-fixture failure; it does not
constitute an all-target green CI result.

The first native observer run at `3b003cf` retained failures in
[run 40010930728](https://github.com/01max/librairii/actions/runs/40010930728).
The X11 private bus lacked the owned display environment during service
activation; the driver now publishes that context only to its private bus and
waits for its GTK backend before starting the public portal. Wayland displayed
the real dialog, but GTK's AT-SPI select-all action returned false. The driver
now focuses the actual file list and uses its native Ctrl+A shortcut, observes
multiple selected rows and still requires both exact returned Unicode paths.
The failed observations remain failures; no gate was relaxed.

The follow-up at `564be95` / run40011756460 proves X11 multiple/single Unicode
selection, parenting, native Settings/Search and owned shutdown. It also exposed
an app adapter bug: RFD0.17.2 represents portal cancellation as `Some([])`, which
the spike displayed as rejection. The adapter now normalizes that empty result
to cancellation. On Wayland, the raw input trace showed only a key release after
keyboard enter, and ephemeral devices removed the seat's keyboard capability.
The observer now keeps a keyboard device on its owned headless seat and allows
focus entry before injecting keys, following the native virtual-keyboard path in
[wtype0.4](https://github.com/atx/wtype/blob/v0.4/main.c).
The multiple-selection precondition uses actual selected rows, since GTK's
selected-child count includes table cells. Exact returned paths remain required.
Both prior native observer runs remain failed; revalidation is required.

At `8a56de7` / run40012639720 all seven X11 picker/focus/early-Settings cases
pass, including cancellation. Reveal failed because the minimal observer lacked
`file`, which `xdg-mime` needs to identify a directory before dispatching its
registered Thunar handler. That runtime helper is now explicitly installed.
Wayland selected-row assertions still fail: the observer now identifies the
actual two-row archive table by its own cell names and selects each real GTK row
through the native table interface, requiring both selected rows and exact
returned paths. This avoids confusing an empty/falsey accessible with the whole
desktop. Native OS key injection is still required for Settings and Search;
no picker keyboard shortcut is claimed by these selection actions.

The `50ac5cf` observer run40013478231 failed to identify the archive table
because it assumed the first accessible column directly exposed filename text.
The observer now inspects each table row's own bounded cell descendants across
columns, requires exactly the two expected filenames in distinct rows, and
retains observed cell names for diagnosis. Native row selections and exact
returned paths remain mandatory; this changes discovery, not the pass verdict.

GTK3's [native table implementation](https://github.com/GNOME/gtk/blob/3.24.41/gtk/a11y/gtktreeviewaccessible.c)
returns false when adding an already selected row and does not implement the
ATK select-all operation. The driver preserves existing selection, adds each
missing row through the native table interface, then requires exactly rows0/1
and both exact callback paths. This follows GTK's actual contract.

At `5747c4c` / run40014481637 both sessions prove exact multiple-file selection
and real parent associations. All seven X11 picker/focus/early-Settings cases
pass. Cold reveal exposed another app adapter issue: `xdg-open` may remain
attached to a newly started Thunar, blocking the synchronous UI callback. Linux
now hands that external GUI to the OS through util-linux `setsid --fork` and waits
only for the short launcher. Request acceptance still requires the observer to
see the real file-manager window/path; no successful launch exit can replace it.
The external desktop app has OS lifetime; synthetic app workers remain owned and
joined, and both test containers retire all remaining desktop processes.

Wayland's actual Ctrl+comma typed a comma. Pinned Slint1.18.1's Winit adapter
tracks modifier key press/release events; a Wayland modifier mask alone is
insufficient. The driver now sends actual Control_L press/release alongside the
native modifier mask, retaining Settings/Search assertions. Raw X11 transient
and PID properties plus the actual app PID are retained for independent review.
Both failures remain preserved pending fresh native revalidation.

At `214bb28` / run40015771459 X11 passes all eight native cases, including the
actual Thunar location and owned shutdown. Wayland's multiple-file case also
passes parenting, exact paths, native Settings/Search and retirement. The next
chooser stalls locating its location entry after Ctrl+L, despite retained GTK
key events. The observer now drains its bounded GLib accessibility-event context
and clears native node caches before traversal; post-shortcut screenshots are
retained. This is an observer synchronization correction requiring native
revalidation, not a declared passing Wayland matrix.

Run40016943055 / `4dd0bd4` falsifies cache staleness as the remaining shortcut
failure: the post-Ctrl+L screenshot has no location entry. That failed GTK
keyboard shortcut stays unqualified. The automation now drives actual GTK file
rows for both single/multiple selection; folder acceptance uses the native
Select button, and rejection activates the real `tmp` breadcrumb, clears child
selection and requires the callback to return/reject exactly the container's
private `/tmp`. Thunar's private location-bar preference is set to entry mode,
read back, and its actual displayed path is still mandatory. No desired path is
written into that location entry.

Independent scope review confirms these native actions satisfy this automated
Task2.6 subset. GTK keyboard-only operation and physical input remain open.
Actual OS Settings/Search key events, all parent/cancel/early-command checks,
both sessions, exact paths and normal owned shutdown remain required. Thunar is
explicitly closed through its private session before retiring the test container.

The separate timeout-fixture correction `229c68f` passes all three native lanes
in [run40019286663](https://github.com/01max/librairii/actions/runs/40019286663),
including both offline runtime and native Linux checks. The Intel test now waits
for child readiness before exercising the unchanged production deadline;
assertions and workloads remain intact. Rust quality and existing CI also pass.

Final evidence head `1c7dae0` retains a separate Intel resource-observation failure
in [run40020211745](https://github.com/01max/librairii/actions/runs/40020211745);
Linux and Apple Silicon pass, as do all Rust quality targets and existing CI.
Intel's normal-renderer warmup completes normally, including its workload and
owned shutdown, but only18 low-latency idle samples survive the unchanged100ms
collector guard (20 required). They span7.201seconds with monotonic CPU. Several
`ps` collections take110–244ms on that runner. All software warmup/measured runs
pass. Artifact12530716647 ZIP SHA256
`ba54ff2b1dbd8910e9845693dafdb580affa719150194974c927fe73bf05052e`
retains the original failing trace. Replaying it through the analyzer reproduces
the same failure and must continue to fail after correction.

Only the synthetic resource probe's quiet phase increases from10 to15seconds to
collect sufficient low-latency observations. A timing model using the recorded
collection costs fails at10seconds and passes at15; it is an authored model,
not a new measurement. The actual native CI probe remains the validation route.
No retry, changed sample/duration/latency acceptance gate, production polling
change or workload reduction is introduced. New reports state the longer quiet
phase explicitly; historical reports and their timing methods remain unchanged.
The analyzer already rejects the incomplete sample set correctly; its existing
short/slow/regressing-sample regressions remain the guard, and native execution
validates the changed app timer. No source-text assertion substitutes for that.

Independent scoped review recomputes the18-sample failure and approves the
longer fixed observation window with unchanged thresholds/deadline. All10
resource-recorder regressions and Rust formatting pass locally. Product
performance budgets remain unqualified. Fresh native CI validates this correction.
