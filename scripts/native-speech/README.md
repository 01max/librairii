This harness shows actual native workflow surfaces backed by isolated production
workers and synthetic libraries. Each workflow/language case owns a fresh private
container, DBus session, desktop, reader, dispatcher and PCM stream, using the same
retained native binary and pinned image. Every case runs in English and French. Keyboard
input is delivered through XTest or the headless Wayland seat; accessibility
queries only observe the resulting names, roles, states, text and focus. They do
not set focus, invoke an accessibility action or synthesize an utterance.

The private desktop and Orca/eSpeakNG/Speech Dispatcher/Pulse capture use the
qualified section-2 per-utterance method v5. Each retained speech witness requires
the expected Orca output after its marker, more than 1,600 nonzero 16 kHz mono
samples and 8,000 trailing silent samples. The 15-second announcement deadline is
unchanged. PCM/WAV, debug offsets, native screenshots, semantic trees, real
accepted/offered command revisions and actual worker-join receipts are retained.
X11 reader action-key echo is disabled in its owned profile; Wayland keeps it.
The private X11 French keymap is set after Openbox registers its persistent
connection, recorded and checked as `fr`/`oss`. The actual compiled keymap must
contain `Eacute`, avoiding temporary keycode remapping for uppercase accented
input. Native dead-key composition and label editing must still produce the exact
expected UTF-8 text in the actual application.
Only Orca gets an isolated XKB compatibility display on Wayland; the application
has no DISPLAY and Sway disables Xwayland. No host audio device is exposed.

`verify-native-speech.py` requires clean committed native Linux source and the
executable retained by `verify-native-foundations.py`. It checks both source and
binary hashes, runs both private sessions without network access, and binds their
reports to that same executable. The complete matrix is exactly112 cases and128
utterance witnesses (64 per X11/Wayland aggregate), including fresh speech after
Shelf reordering and tag assignment while that same app/reader is still alive.
The aggregate30-minute budget is shared by all56 independent cases in a session;
the native application's180-second deadline and all per-utterance bounds remain.
Every case must positively join its real worker. Raw reports/audio/debug offsets
remain per case and are not concatenated into synthetic utterance streams.
The container's read-only repository mount is
needed because the native fixture uses compile-time absolute corpus paths.
The typed `finish` request exists only inside explicitly marked fixture output;
the native process still stops and joins its real worker before writing `closed`.

These receipts cover workflow presentation, keyboard reachability, focused spoken
names and representative actions. Existing callback, storage, fault, visual and
AT-SPI fixtures remain mandatory. Hardware input, the owner-waived VoiceOver/IME
executions, large-library numerical budgets, installed packages and final cutover
retain their distinct qualification boundaries. Saved concept source is reviewed
without bypassing the blocked preview URL.

After a visual-step failure, CI still records spoken diagnostics when the retained
binary matches Cargo's compiler-artifact receipt. Those diagnostics explicitly
record native_visual_passed=false and complete=false. The visual failure remains
a failing CI step. Final complete acceptance requires its sealed visual manifest.

Independent cases continue after an ordinary case failure in their own fresh
containers; every missing/failed case or absent positive native join keeps the
aggregate failing. Timeouts remain fatal and explicitly retire the named owned
container. Speech Dispatcher retains warning/error diagnostics at level2; Orca
INFO output and per-utterance PCM remain required. The image supplies and records
the standard platform certificate bundle used by the unchanged TLS verifier.

Failed run40593296056 remains diagnostic evidence: both reused-reader sessions
stopped producing utterances after the Shelf reorder/exit boundary, with repeated
late reader stacks in `AXObject.get_application` through debug/event handling.
That trace does not establish whether mutation or exit caused the stall. The new
same-reader post-reorder witness must pass; isolation makes no general claim about
continuous installed-package or reader behavior across application restarts.

Run40599944235 at head8250d32 is failed diagnostic evidence. Its X11 reader
retained controls marked defunct after they temporarily scrolled out of view.
The promoted adapter now distinguishes filtering from actual logical deletion;
the historical qualified sources and the disabled/Sensitive backport remain.
Fresh source-bound Linux reader and all three native target receipts are required.

Before each target utterance, native keyboard input leaves that control. The
observer requires Orca's debug log to acknowledge the actual new/current neighbor
locus, with its complete name and role; an old-locus name alone cannot satisfy
this handshake. It then runs fixed `spd-say --cancel` against only the case's
private `SPEECHD_ADDRESS` to discard queued navigation speech, records completion
and offsets, and observes actual PCM silence before marking and returning to the
target. Cancellation occurs before the target marker and never during its spoken
witness. This measures focused announcements, not uninterrupted document reading.
The retained round-trip records preserve handshake and cancellation failures.
Parser regressions run when the owned image is built. All original target-word,
audio, focus, worker-join and time bounds remain unchanged.
