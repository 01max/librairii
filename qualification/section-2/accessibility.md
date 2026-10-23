# Accessibility / keyboard gate (2.4, P56–P57)

2026-10-23 scope update: the owner explicitly declined VoiceOver testing.
VoiceOver execution is skipped for this Task 2 qualification and does not block
this task's current selection/merge gate. Spoken macOS accessibility remains
unqualified. Keyboard/AX checks and the remaining Linux checks retain their
scope. Current ARM input observations are in [native-picker-input.json](native-picker-input.json).
The initial access-denied inventory below is historical.

Status: **not qualified on any target**. Observer: librairii-4, 2026-10-20.

ARM Slint process execution succeeded, but native Computer Use rejected
`Qualification.app`: “Computer Use was not approved to use Librairii Qualification”.
The unbundled binary was not an addressable native application. No keyboard input,
AX tree, screenshot, VoiceOver speech, NVDA or Orca result was observed. No privacy
permission or OS accessibility setting was changed to bypass the access boundary.
The orchestrator received the precise request for approved native app access or
a human operator, in addition to Intel/Linux/Windows hosts.

The spike currently uses standard buttons/edit/checkbox/progress controls plus
virtualized list/grid rows. Those source choices do not establish semantics.
Known incomplete probe surfaces: real modal role/focus containment/restoration,
progress/error announcements, 300-candidate review with 25 mounted decision rows,
and narrow sidebar disclosure. The modal-looking rectangle is not qualified as
a dialog. This is unfinished spike work, not an established toolkit failure.

For **each** target, record commit/binary SHA-256, host/OS, renderer, reader exact
version, operator and capture references, then observe:

1. Tab/Shift-Tab to all actions, visible focus and correct ordering; select a
   virtualized offscreen story by keyboard, change density, preserve selection.
2. Open/close details/settings and review from different invokers; verify initial
   focus, containment, Escape, nested dialog behavior and exact return target.
3. Inspect names/roles/selection/checked/disabled/mixed states and offscreen
   semantics with VoiceOver (ARM and real Intel), NVDA, Orca/AT-SPI (X11/Wayland).
4. Hear progress, cancellation, completion and an injected error once; ensure
   repeated updates do not flood speech and terminal state stays discoverable.
5. Complete the synthetic workflow without pointer assistance. Attach observations
   of actual spoken output and focus changes; screenshots/controller tests alone
   cannot pass this gate.

Missing evidence remains A/I/L/W pending. Preserve failures rather than changing
the checklist. A reproducible mandatory toolkit failure activates 2.12.
