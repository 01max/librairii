# Input / display gate (2.5, P14/P53/P56)

## Automated macOS composition continuation (2026-10-24)

`check-input-macos.py` is restricted to the disposable hosted native macOS CI
account, with an additional explicit step environment flag. The local observer
cannot mutate input sources without that route. The matching archive and every
payload are verified before launching. Public Carbon TIS APIs select the exact
installed French layout and Japanese Roman-input family; public CGEvents post
physical key codes without Unicode payloads or programmatic field values.
Native foreground PID is required for every key command.

French requires actual `^` + `e` dead-key composition plus accented/AZERTY
positions to produce exact `êé az`. Japanese requires real nonempty preedit,
candidate conversion and exact `日本` commit, followed by a second preedit which
is cancelled without any commit. A versioned public Winit event hook records
the actual IME events and always propagates them unchanged. The feature is
explicitly named `unstable-winit-030`; it is pinned with Slint1.18.1 and must be
rechecked on upgrade. No private toolkit API or commercial test tool is used.
Native screenshots retain composition/candidate/committed states for review.

The driver records original source/enable states before any mutation, restores
the original selection and every newly enabled mode/parent in a `finally` block,
and verifies the effective inventory afterward. Restoration failure is fatal,
including when input itself failed. All source changes remain in the existing
ephemeral CI account; the owner's keyboard/IME/privacy settings are untouched.
Normal worker shutdown, fixture retirement, evidence/command bounds and the
existing45-second app deadline remain required. First actual native execution
and independent evidence review are pending; this method alone is not a pass.
Linux composition and the broader display/contrast/scaling gates remain open.

## Historical initial checklist

Status: **pending actual observation on A/I/L/W**, 2026-10-20, librairii-4.
Native input access was rejected for the spike app (see accessibility.md).
Synthetic French/Japanese/Chinese strings exist in memory and the process runs;
this does not establish rendered glyph correctness, composition or text editing.

Record OS, keyboard layout and IME exact versions, monitor models/scales, renderer,
app commit/hash and operator. Use only synthetic filenames such as
`Été é 日本語 故事.zip` in a temporary folder. Observe and retain:

* French AZERTY and dead-key sequences, composed/decomposed text, Japanese/Chinese
  preedit/candidate selection/commit/cancel; typing whitespace must remain intact.
* Select all, copy/cut/paste/undo/redo, caret movement, selection and clipboard
  roundtrip through the native editor. Pasting Unicode is not an IME test.
* Long French labels; narrow 560 px / intermediate 850 and 1180 px / short windows;
  independently scrolling areas and reachable navigation. The current spike keeps
  desktop panels present; narrow disclosure is unfinished, not a passing layout.
* 100/125/150/200% text/display scaling as applicable, real mixed-monitor moves,
  resizing during work and dialog ownership; record clipping and scale transitions.
* High contrast, focus visibility, non-color identification and OS reduced motion.
  Static source/no animation does not verify OS preference integration.

Do not change the user's system settings or keyboard configuration without the
agreed interactive test setup. Missing monitors/IME/operator access remains an
explicit resource requirement, not a simulated pass.
