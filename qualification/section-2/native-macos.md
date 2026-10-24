# Native macOS interaction observer (2.6, partial)

The observer runs only against the byte-verified, native packaged spike. The
Swift helper uses public AXUIElement, CoreGraphics event and AppKit clipboard
APIs, with existing CI permissions. It never requests or modifies privacy
permissions. Input is refused unless the explicit target PID is frontmost.
Native control selectors retain role/title/identifier fingerprints, so a changed
tree cannot silently redirect an action. All commands/output are bounded.

Each architecture must independently complete eight cases: multiple/single file
selection, file cancellation, folder acceptance/cancellation/outside-root
rejection, early Settings and actual Finder reveal. Picker parenting requires
an actual public AXWindow child-sheet relationship, with raw tree evidence.
Selection uses native row controls and exact returned paths; folder navigation
uses the real Go-to-Folder control. Captures do not independently prove the final
row highlight. After pickers close, OS focus must return without reactivation.
Native Command-comma, Settings menu, Escape and Search are exercised on every
case. The early case also checks native Unicode paste/cut/clipboard restoration
and the Edit menu. Every app must normally join workers and remove its fixture.

Reveal requires Finder's actual document URL at the owned fixture, rather than
successful request acceptance. Only that opened Finder window is closed. The
observer explicitly returns to the application afterward; it does not label
that external-file-manager return as automatic picker focus restoration.

Actual native execution remains pending. Compilation/recorder tests are not
interaction evidence. These are synthetic OS events/native control actions,
not physical keyboard latency, dead keys, IME composition, VoiceOver speech,
mixed-monitor or clean-host qualification. VoiceOver remains excluded from the
current gate by the owner's decision; Windows remains deferred.

The first ARM experiment at `b71dcb2`,
[run40075891289](https://github.com/01max/librairii/actions/runs/40075891289),
retains an actual pre-dialog failure: invoking AppKit's modal panel directly
inside the automatic Slint timer re-enters timer processing and panics with
`Recursion in timer code`. Artifact12552957741 ZIP SHA256 is
`ff06abb6ecf7b583ea3e53513c1213f3abbda380e79321aabaf3d7458b6a99cb`.
The test rendezvous now queues its native callback through Slint's public
event-loop API after the timer returns. Native parenting, exact callbacks,
OS events and every observer assertion remain required; fresh native execution
must establish whether that correction works.
