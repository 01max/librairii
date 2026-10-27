This harness shows actual native workflow surfaces backed by isolated production
workers and synthetic libraries. Every case runs in English and French. Keyboard
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
Only Orca gets an isolated XKB compatibility display on Wayland; the application
has no DISPLAY and Sway disables Xwayland. No host audio device is exposed.

`verify-native-speech.py` requires clean committed native Linux source and the
executable retained by `verify-native-foundations.py`. It checks both source and
binary hashes, runs both private sessions without network access, and binds their
reports to that same executable. The container's read-only repository mount is
needed because the native fixture uses compile-time absolute corpus paths.
The typed `finish` request exists only inside explicitly marked fixture output;
the native process still stops and joins its real worker before writing `closed`.

These receipts cover workflow presentation, keyboard reachability, focused spoken
names and representative actions. Existing callback, storage, fault, visual and
AT-SPI fixtures remain mandatory. Hardware input, the owner-waived VoiceOver/IME
executions, large-library numerical budgets, installed packages and final cutover
retain their distinct qualification boundaries. Saved concept source is reviewed
without bypassing the blocked preview URL.
