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
