# Linux package runtime isolation (Task 2.11)

`check-clean-linux.py` verifies the native Linux archive/package bytes, then
launches that extracted executable in a separate Ubuntu 24.04 amd64 container.
The base is pinned to official manifest
`sha256:a2d43bffc46670d7108bb3c0468c67c5557c4c92e83e1e34cbff1322ad1b94dc`
(resolved 2026-10-23). The [Dockerfile](clean-linux/Dockerfile) declares runtime
libraries/fonts and the separate Xvfb/Weston/DBus observation tools. There is no
compiler, Rust, Go, Node, Python, browser or source checkout inside the container.
The Python recorder and Docker client run outside the application environment.
Package versions and the built image ID are retained, rather than claiming that
a mutable Ubuntu package archive was frozen by pinning the base alone.

The app runs as an ordinary user, from `/tmp`, with an empty inherited environment
apart from explicit session/runtime variables. The container has no network,
read-only root, no capabilities, no privilege escalation, and only two host bind
mounts: the verified package read-only and fresh evidence writable. The app's
temporary fixtures/cache/config live on a temporary filesystem. Neither the
checkout nor host toolchains/display sockets are mounted. No host driver changes
or production profiles are used. The container and task-specific image are removed.

Normal and explicit Skia CPU requests each execute on an owned X11/Xvfb server
and an owned Weston headless/Pixman Wayland compositor. Each run must identify its
actual rendering surface, produce fresh distinct French/CJK snapshots and pass
Search component/model checks. Wayland additionally requires configure/ack and
non-null buffer/commit on the same toplevel. Each session then launches a fresh
CPU process and verifies active-worker cancellation/join/fixture retirement.
Explicit CPU selection must report the actual Skia software surface.

The host checks actual Docker isolation settings, native architecture, ordinary
UID, absence of development/browser packages, resolved linked libraries, matching
executable hash and every required run/cleanup result. Missing evidence or a
failed command fails the recorder. Build and execution have separate 900/300-second
bounds; each app has a 30-second bound. Logs, exact commands, image/source/archive/
binary identities, container inspection, package inventory and PNG hashes remain
in `clean-linux/clean-linux.json` and the native CI artifact. Container-side captured
logs are capped at 4 MiB through pipes (reaching the cap fails the probe); the recorder also rejects oversized files or more
than 64 runtime evidence files before reading their contents.

Native execution at implementation `3b003cf` passes all 16 checks in
[run 40010930728](https://github.com/01max/librairii/actions/runs/40010930728).
[Retained observations](clean-linux-observations.json) bind source, image, binary,
archive, artifact ZIP, raw report and all runtime-file hashes. Independent review
recomputes the verdicts, checks all 27 file hashes and visually verifies all eight
French/CJK frames before/after filtering. Both Wayland routes configure, acknowledge
and submit buffers on their actual toplevels. Normal reports WGPU; CPU reports
software. This qualifies an isolated Linux runtime environment for this synthetic
spike only. It does not prove a complete desktop
install/update/uninstall, native portal/reveal integration, physical input/display,
all minimum-OS configurations, or either macOS clean-host route. Those gates and
overall Task 2.11 remain open. No toolkit selection or public release claim follows.

The initial native experiment at `0e17aa6` is retained in
[run 40008230875](https://github.com/01max/librairii/actions/runs/40008230875).
It failed before starting the app: DBus rejected the mapped runner UID 1001
because the base image had no matching passwd entry. The correction creates an
ordinary account/group matching the numeric runner UID/GID while building the
test image. Execution remains nonroot and offline with the same restricted
mounts/capabilities. No missing-user error is relabeled as a runtime/app pass.

The follow-up at `7e9f3f4` started DBus successfully but failed with SIGXFSZ
before the first frame: the inherited file-size limit also restricts graphics
buffer files, not just log output. Its raw evidence remains in
[run 40009932756](https://github.com/01max/librairii/actions/runs/40009932756).
Log capture now uses bounded pipes, preserving graphics buffer allocation while
still rejecting capped output. The temporary runtime filesystem remains capped.
