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
selection remain open until their remaining scope has evidence. Native execution
of this new observer is pending; unit tests do not establish integration success.

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
