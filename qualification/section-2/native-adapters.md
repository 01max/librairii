# Native adapter probes (2.6, P05/P48/P53/P57)

Task2.6 is complete: all eight native cases pass on each Mac target and each
Linux desktop, including exact UTF-8 selections/cancel/reject, parent association,
Settings/early commands, native clipboard and validated Finder/Thunar reveal.
See the attributed native-macos/native-linux records and [selection-gate.md](selection-gate.md).

Earlier observations below retain their dates, source commits and historical scope.

Status: **probe implemented; actual dialog/menu/reveal behavior pending**.
The GUI now has rfd multiple-file/folder probes parented with Slint's public
`Window::window_handle`, a native Slint menu with Settings/About, a simulated
`--settings-early` command, and a reveal request for an owned temporary folder.
The startup flag is not evidence of OS command buffering. Platform accelerators
and standard edit/app menu roles remain unfinished.

Each launch creates only a disposable picker fixture with an artificial Unicode
filename. Choices outside that root are rejected; no selected bytes are imported.
Reveal validates the owned directory and passes it as a separate process argument
to the platform file manager. A successful process exit is not proof the correct
window/path appeared. No production root/profile is used.

The native interaction access denial is recorded in accessibility.md. No picker
selection, cancellation, modality, menu invocation, accelerator or file-manager
observation is claimed on A/I/L/W. Probe compilation alone cannot close 2.6.

On each qualified OS/session, capture operator, commit/package hash, desktop/file
manager version and the observed selected paths in the disposable root. Exercise
two files, one file, folder, cancel, wrong-root rejection, parent focus while a
dialog is open, close/reopen, language changes, Settings before/after readiness,
all conventional editing/Settings accelerators and validated reveal of the exact
directory. Linux needs both portal-backed X11 and Wayland observations.

## Automated Linux continuation, 2026-10-23

See `native-linux.md` for retained real GTK portal selections/cancellation, exact
parent proof and native focus/input coverage. Earlier access-denial notes above
are historical; these automated sessions do not qualify physical input or speech.
The Linux reveal probe needs util-linux `setsid`, `xdg-open`/`xdg-mime`, the `file`
directory-detection helper and a registered desktop file-manager handler. It
validates the fixture path, passes separate arguments, and waits for the short
OS launcher rather than the external file manager's lifetime. Only an actual
observed window at the correct path can qualify reveal; request acceptance alone
cannot. No new Cargo dependency or toolkit pin is introduced.

Implementation `230cc47` now passes the automated Linux X11/Wayland matrix: all
16 cases, with exact results and actual file-manager paths. See
`native-linux-observations.json` for source/binary/artifact binding and scope.
Physical/keyboard-only/cross-platform qualification remains open.
