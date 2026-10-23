# Automated packaged Wayland qualification (Task 2.11)

`check-wayland.py` runs the manifest-verified native Linux x86-64 executable
against an owned Weston headless compositor with its Pixman renderer. CI uses
Ubuntu 24.04's signed packages and records their exact versions. No new Rust
dependency or shipping graph is introduced. The compositor is a test dependency.
Weston's [headless backend and Pixman renderer](https://wayland.pages.freedesktop.org/weston/toc/running-weston.html)
provide an actual Wayland protocol session without a physical output.

Run on native Linux as an ordinary user with Weston, wayland-utils and DBus:

```sh
dbus-run-session -- python3 qualification/section-2/check-wayland.py \
  --package /absolute/path/to/extracted/task2-x86_64-unknown-linux-gnu \
  --output /absolute/path/to/new/wayland-evidence
```

The recorder creates a private mode-0700 runtime directory and isolated config
and cache directories; it removes inherited DISPLAY/XAUTHORITY, Wayland socket
FD and GPU-failure overrides from child environments. It starts Weston with an
explicit owned socket, headless backend, Pixman renderer, kiosk shell, no user
configuration and no idle timeout. Xwayland is not enabled. `wayland-info` must
connect and enumerate compositor, output and xdg-shell protocols before probes.
Both the normal Skia choice and explicit Skia CPU choice must configure an xdg
window, acknowledge its configuration and attach a real `wl_buffer`, then
commit that same toplevel's `wl_surface`. Cursor/subsurface submissions cannot
replace this proof. Evidence comes from `WAYLAND_DEBUG=client` through the pinned
wayland-backend 0.3.17 (Winit selects its system/libwayland client backend)
trace, not session environment labels. The explicit CPU run must also report
the actual Slint Skia software surface. A deliberately absent Wayland socket
must fail with no X11 fallback available; unrelated positive-case failures
cannot turn that negative control into an overall pass.

Each rendering run saves two fresh, distinct French/CJK scene snapshots and
verifies Search component/model updates. Slint's snapshot API re-renders on CPU;
those images alone do not prove a Wayland window was submitted. The separate
protocol checks establish submission, not visible/physical presentation.
Additional CPU-path component checks cover editing/Undo/Redo, modal focus and
restoration, active-worker Quit/join/fixture retirement, and 1,000 PNGs with the
existing bounded cache. These are public-API synthetic inputs; physical keyboard,
clipboard, dead keys, IME, speech and hardware input latency remain unqualified.
The kiosk shell also does not qualify desktop window resizing/mixed monitors.

Every command has a 30-second deadline and 4 MiB retained-output bound; compositor
startup has a ten-second deadline and its log has the same retained size cap.
The recorder kills owned process groups, reaps direct children, rejects surviving
descendants, and stops the compositor before removing its runtime directory.
Failures retain raw logs and the report; output directories cannot be reused.
Source/executable hashes, exact commands, tool versions, per-check verdicts and
PNG hashes are retained in `wayland/wayland.json` and the native CI artifact.

Execution is pending this continuation's native CI. Passing these probes supplies
automated Wayland transport/rendering/component evidence only. Task 2.11 remains
open for complete minimal-package/clean-host qualification; Task 2.6's portal
pickers/reveal, Task 2.4's Orca/AT-SPI and Task 2.5's OS-input/IME/scaling evidence
are separate unfinished gates. Toolkit selection remains open.
