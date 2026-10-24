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

Reveal requires an actual native filesystem location witness in the owned Finder
window: either its root file URL or both exact existing fixture files' absolute
URLs inside one native IconView with the exact fresh fixture title. The latter
handles Finder15's empty root URL attributes; a title alone never qualifies.
Only that freshly reverified Finder window is closed, and absence is observed. The
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

At `aab701c` / run40080362178 ARM completes the five file and folder
acceptance/cancellation cases. Folder rejection times out during AX traversal
after Go to Folder navigates into the system temporary parent, whose unrelated
contents exceed the bounded observation route. The rejection case now navigates
to a fresh, empty, owned evidence directory outside the accepted fixture root.
The actual picker must still return that exact canonical directory and the app
must reject it. No timeout or assertion is relaxed. ARM artifact12555145869 ZIP
SHA256 is `32f47a1cc989b6f6e7bdd2b9bc2eb98efaba87995d75ede2ee921f6e4d520ef4`.

At `5de32ce` / run40081026276 ARM completes all seven picker/early-Settings
cases, including exact outside-root rejection and Unicode native clipboard/
Edit-menu behavior. Reveal returns request acceptance, but the observer never
finds a matching Finder window. Artifact12555470774 ZIP SHA256 is
`eb8795f262023510b819fda44aab50f738a319b71f9b5640e5f30678b871a804`.
The observer now checks both public AXDocument and AXURL local-file attributes
against the exact fixture, rather than AXDocument alone, and retains the count
of Finder windows plus root metadata only for the exact owned fixture title if
URL matching fails. Title alone cannot pass; no unrelated folder is traversed
or captured. The cause and full reveal result still require native execution.

At `4ea1e4b` / run40081851994 ARM again passes all seven preceding cases. Finder
has an actual window with the exact fresh fixture title, but both public root
URL attributes are empty. The observer now retains a diagnostic tree only for
that unique owned-title window, separately from qualifying nodes/captures; this
cannot pass reveal or authorize cleanup without an actual path witness. It will
identify the native path control rather than treating the title as a path.
Artifact12555656307 ZIP SHA256 is
`6ec1cdaf9c523374e36b8c95398c1da1d177132a6e54323dec3f14f065fb3f77`.

At `eca64c4` / run40092596117 the owned107-node ARM Finder tree identifies both
fixture files as native AXImage children of the same IconView, each with its
exact full file URL. The current observer verifies both existing canonical paths,
their shared fixture parent, titles, owning window and native content-view
relationship before using the candidate tree or capture metadata. Duplicate,
missing, cross-window, wrong-directory and out-of-content witnesses fail. Cleanup
rechecks that same path proof before pressing its Close control and waits for
that owned window to disappear. This is a content-file-URL location witness,
not a claim that Finder exposes a root AXDocument URL.
ARM artifact12560040145 ZIP SHA256 is
`a0c2b0395a476c16344384af31ff489f8a6b1f8d58ab9aa4acf7f265a235698a`.

The same run's Intel folder-acceptance snapshot records an Open button still
disabled after Go to Folder closes. The observer now waits for the unique actual
button to become enabled before its single press, within the unchanged bound;
multiple matching buttons fail immediately. Intel artifact12559661407 ZIP SHA256
is `808face7e282934af6df43be67ebd40ac7f3cb926748e3fe2f4dfb482037a0db`.
Independent review also requires a successful AXWindows/title query before
recording Finder absence. Query failures remain command failures, and an explicit
present flag prevents an empty traversal from masquerading as a closed window.

At `6da2838` / run40093774916 ARM again completes the seven picker/early cases,
then Finder's initial AXTitle read returns `noValue` (-25212) before path
verification. ARM artifact12560037740 ZIP SHA256 is
`305bf1d16aae3435d4483bba7d69893bfdd50aab2576ab831b31b4b8365359eb`.
The observer now records unresolved titles as incomplete read-only observations
within the existing wait. It cannot record window absence while any title is
unresolved; real AX query failures remain fatal. Exact path proof and verified
closure remain required. Apple's [AXError.noValue](https://developer.apple.com/documentation/applicationservices/axerror/novalue)
means the requested value or element does not exist. This readiness correction
still requires fresh native execution; the earlier reveal case stays failed.
The same run's Intel lane stops in the recorder suite before packaging or
interaction; its separately reproduced process-cleanup race is in `ledger.md`.

At `4260059` / run40094467972 ARM verifies the exact native content-file URLs
and executes the verified window's Close control. The remaining untitled Finder
window makes the all-titles-known absence method time out. Artifact12560695759
ZIP SHA256 is `5aec94e5648bb03ffa3b48326af685694628a6cb21313ba5baab3b8ee34c63d9`.
The closure method now retains the unique public CG window ID from the already
path-verified window, rechecks that same identity before Close, and requires its
absence from a successful complete CG window list, including offscreen/minimized
windows. A successful AXWindows query must also find no exact document candidate
or path witness. An unrelated unresolved title cannot replace this independent
window-retirement proof. Missing/failed queries, a changed ID or a still-present
window fail. The original timeout remains failed; fresh native execution is
required. No untitled window is acted on or traversed for this correction.

Intel independently completes all seven picker/early cases in the same run.
It proves the exact Finder content path before CG publishes the matching window,
so its first capture has no target. Its next retained snapshot has that unique
CG window. Readiness now waits for both exact AX content and unique owned CG
identity before capture; ambiguity remains fatal and no failed action is retried.
Intel artifact12560069276 ZIP SHA256 is
`4ebf360b49abc95f789b7e895be6a4e04fc0be8454f985ed13a110b0e501e073`.
