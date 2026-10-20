# Input / display gate (2.5, P14/P53/P56)

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
  independently scrolling areas and reachable navigation. Existing sidebar hiding
  below 850 px without disclosure is an unfinished probe, not a passing layout.
* 100/125/150/200% text/display scaling as applicable, real mixed-monitor moves,
  resizing during work and dialog ownership; record clipping and scale transitions.
* High contrast, focus visibility, non-color identification and OS reduced motion.
  Static source/no animation does not verify OS preference integration.

Do not change the user's system settings or keyboard configuration without the
agreed interactive test setup. Missing monitors/IME/operator access remains an
explicit resource requirement, not a simulated pass.
