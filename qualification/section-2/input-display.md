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

First execution at `3cef8c1` in
[run40100102140](https://github.com/01max/librairii/actions/runs/40100102140)
retains the same observer setup failure on ARM and Intel, before source mutation
or text input. The optional legacy Kotoeri bundle has no matches, and Carbon
returns nil rather than an empty array. Both existing eight-case native-adapter
runs pass; ARM's failed input run still exits0 and retires its fixture. ARM
artifact12562189090 has ZIP SHA256
`a9d14f40ec6490d12f504f4e5147af46a2e57b23364f4e5b93f38f1455968ef6`;
Intel artifact12562706182 has published ZIP SHA256
`2b619a34ac18b9359368e53cf04ad892340254d3a60b9c27a1b0d220cf3b9191`.

The separate `0ad3f29` correction treats nil as an empty inventory, while exact
French/Japanese identity and availability remain mandatory. A real read-only
Carbon regression exercises an absent family, the installed French layout and
unchanged original selection. All91 recorder tests pass on macOS; the one Carbon
test is explicitly native-only on other platforms. Independent review approves
the correction; fresh native input execution remains required. No failed run is
retried into a pass or treated as a toolkit failure.

## Native editing diagnostic method v2

At0c6c073/run40100937198 both Mac input runs progress past inventory and fail
clearing the French text after native Select All/Delete. The inspected ARM raw
record proves exact `êé az`, real Winit `Preedit("^")` / `Commit("ê")`, then
`êé a` after Delete. All original source/enable bits restore and the app exits0
with its fixture gone. ARM artifact12563420242 ZIP SHA256 is
`a3e3bfa15f997e322bf02b34b7a1f20528678fa191f920bbde226a35e6a508b9`.
Intel's same-stage failure is retained in artifact12563016813, published ZIP SHA256
`d0dee38fd4e6311f3fae221155eba477293dce15a92b958d9788ad45bbfcb158`.
Linux's full lane passes. The failed Mac runs do not establish Japanese input.

The v2 method records native menu enabled metadata, Muda event receipt/enqueue/
execution, the actual widget edit guard, and propagated native focus/key/IME
transitions. Before posting Delete it reads the owned app's focused AX text,
selected text and UTF-16 selection range, requiring one complete selection from
one Select All command. Wrong/partial text, range, focus PID or missing selection
cannot pass. It never sets selection through AX, retries the action, changes
focus to satisfy a check or bypasses the edit guard. This distinguishes missing
command delivery, a guarded no-op and later selection loss before changing app
behavior. Existing deadlines, output bounds and restoration remain required.
Atf462e38/run40102132125, ARM proves native Muda receipt, enqueue, execution
and an allowed Search edit guard after exact French input. The first app-root
AX focused-element query returns no element, and the observer aborts before
Delete. This does not establish a Slint selection defect. The raw31 ARM input
file hashes and29 Intel input file hashes verify against their reports.
ARM artifact12563427274 ZIP SHA256 is
`ff8a293a3becdf13c61101adcf83ec5c19c0db570ded9d290bf425f4f394cffd`;
Intel artifact12562987657 ZIP SHA256 is
`1b3d70b7e46c5056bf450a4d025e13805946a8fa8cdb312edc71908bc60870af`.
Both input runs restore original source states, exit0 and retire their fixtures.
Intel separately fails Select All lookup, and its adapter run retains a Finder
AXWindows `cannotComplete` failure; neither result is relabelled a pass. Linux's
full lane and both quality workflows pass at this head.

The focused-element reader now preserves exact native status and a bounded,
read-only owned-window tree snapshot when initial focus is absent. AccessKit's
pinned macOS adapter can request its initial tree asynchronously on first
access. The existing bounded poll may observe the later tree; it never treats
absence as selection, changes focus, sets selection or repeats Select All.
Actual `AXFocused=true` is required in addition to the full text/range/PID checks.
This observer correction still requires fresh native execution; the original
failed runs and the earlier partial-delete symptom retain their verdicts.

The separate Intel menu-reader correction explicitly opens the one native Edit
menu before looking for its Select All child, allowing AppKit to expose lazy
submenu children. It performs the item action once and records whether the
submenu opened before lookup; an absent/ambiguous menu or missing item still
fails. General adapter-menu behavior is unchanged. Fresh ARM/Intel execution
must establish delivery and selection; the f462e38 Intel failure remains.

At5f1606d/run40103474086 the ARM reader first records native `noValue`, then
the actual focused Search field with exact selected `êé az` and UTF-16 range
0+5; one physical Delete clears the query. Japanese input now reaches real
Winit preedit `n` → `に` → `にh` → `にほ` → `にほn`, but the driver incorrectly
waits for `にほん` before finishing the pending final Roman n. It times out,
restores every original source/enable bit and exits0 with its fixture removed.
All retained input file hashes verify. Artifact12562714907 ZIP SHA256 is
`39c94122d498169a76d61148c4e36b43eb770c24c3459d10a54dfa6c8d4c83c8`.
This is actual Japanese preedit evidence, not a completed conversion/cancel pass.

The separate sequence correction types `nihonn` in both Japanese phases so
the input method completes the final kana before the exact `にほん` assertion.
No Unicode payload, application text setter, IME API shortcut or weakened
expected text replaces physical key input. Candidate `日本` commit, a fresh
cancelled composition, full restoration and normal retirement remain required.

## Input method service readiness (method v3)

The5f1606d Intel run also verifies exact French full selection and clearing,
then records TIS Japanese selection but native Winit `Disabled` followed by
ordinary Latin `nihon` input, without Japanese preedit. This is distinct from
ARM's pending final Roman n. Intel artifact12564410621 ZIP SHA256 is
`5368681dc1517cee901825ed6bbdea3bb842f325b0f9ec8db0979d41fddc7165`.
It retains the same-stage timeout, restoration and normal retirement.

The v3 observer separately records public NSRunningApplication state for the
exact Japanese Roman-input service identities in a two-second read-only launch
observation window before typing. It records selected TIS mode plus any actual
live application which has posted its launch notification. Apple's source
family identifier and application bundle identifier differ; both exact known
identities are read, and no process is launched or killed through this check.
Some applications never post that notification. Its absence is recorded as an
incomplete optional observation and does not veto or qualify IME capability.
Every actual preedit/candidate/commit/cancel assertion still runs afterward.
This setup diagnostic requires fresh native execution and does not claim that
service startup caused Intel's failure. No additional capability gate is added.

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
