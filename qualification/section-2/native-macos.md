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

The same run's Intel observer also fails before observing an attached sheet:
its bounded AX snapshot command times out while the app remains blocked. No
timer panic is present in its retained app log, so that separate failure is not
attributed to the ARM panic. Both its permission inventory and native result
are retained in artifact12553497370, ZIP SHA256
`625322f3ea8f936995d41cafa4600eb68b168d816c49efda24b63aa791cd8ea5`.

The next ARM case at `da29810` / run40076893068 no longer records a timer panic,
but its first full AX-tree snapshot exceeds the12-second command bound. The
app's rendezvous still reports live Search state. Artifact12553935368 ZIP SHA256
`94734586dd77c9227adbb292caf785140fca48f6ddcb25fe13f122a21fc8bc40`
retains this failure. Foreground/window status now uses only public OS metadata;
the picker observer traverses attached native sheets rather than redundantly
walking the full Slint content tree and its sheet again. Finder remains limited
to the exact fixture document. Captures choose the frontmost visible owned
window, including modal levels. Full Slint accessibility-tree traversal and
spoken semantics are separate, still-open2.4 evidence; no result is inferred
for them from this targeted native-adapter observer.

At `cf58c8e` / run40077600508 the ARM observer reaches and proves the actual
parented native sheet, then fails file selection. Its raw tree identifies the
observer's broad editable-field fallback selecting a filename cell while the
Go-to-Folder sheet remains open. The fresh-case observer now requires the actual
`GoToWindow` child sheet's `PathTextField`, verifies its exact value and waits
for that sheet to close before selecting files. Search and filename cells are
explicitly rejected by a regression. No failed dialog is retried.
Artifact12553273734 ZIP SHA256 is
`450067c082d23c1aeb61a9f5a31724b596e2ee6fbf088bb0c1ced78a772bbff3`.

At `4861fe7` / run40078264703, ARM selects both exact Unicode paths successfully,
then the unchanged45-second probe guard expires before later Settings checks.
The observer repeatedly traversed a572–582-node column browser, including all
ancestor-directory columns. It now proves the attached sheet from native roots,
switches to the observed133-node List view before navigation, reuses snapshots,
and reads the actual path value directly after setting it. Native row sets,
callbacks, parenting, unforced focus and every required case remain mandatory;
neither the12-second command bound nor45-second app bound is extended.
Artifact12553653602 ZIP SHA256 is
`fba4a11aab2bc8a9d55e912ec4acfb274cc21e7377ccb17bd46084efe54eac12`.

At `4e63cc0` / run40078900422, ARM proves parenting, both exact selected paths,
unforced focus restoration, Command-comma and the native Settings menu. Search
remains unchanged after the observer posts Unicode payloads on a synthetic A
key. This delivery method is unqualified; the result is retained in
artifact12554487269, ZIP SHA256
`328344a40e11b25e027153834b040e0062fb7c6d8d1f485628658fce0bce25cc`.
The numeric Search check now posts actual native number-key down/up events with
explicitly empty modifiers and records the current public keyboard-source ID.
Exact `0042`/one-row observation remains required. Unicode paste/cut/Edit-menu
checks still use the real system clipboard; no IME/dead-key result is inferred.
Intel independently passes the same five checks and retains the same unchanged
Search state in artifact12553963227, ZIP SHA256
`005862a6314b15040c85d0b5ff2158f1a4414cf01983846f357994c4dc2b7c9b`.

At `c59c0bd` / run40079709208 both architectures complete all checks for multiple
files, a single file and file cancellation, including native number-key Search.
Folder acceptance returns `selected` and the correct fixture through macOS's
`/var` alias, while the fixture rendezvous records `/private/var`. The observer's
text-only path comparison rejects that alias. It now resolves both existing
paths strictly before fixture retirement and records raw and canonical values;
counts and outcome remain exact. Missing, relative and different paths remain
invalid. ARM artifact12554623051 ZIP SHA256 is
`e6ac3c9ca3ac4ed6341479f82317e9e85237b4304c297e5e0362d397d48e000a`;
Intel artifact12554228106 ZIP SHA256 is
`d116219388551aa5da7a9be2629f540ae9411f2fdae165047e91e98dceb5f512`.
