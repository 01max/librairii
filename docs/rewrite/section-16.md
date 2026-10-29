# Scale, resilience and resource qualification

Task16 starts from integration `a22dd5bb61741472b7ce80ac5aabb057e3edbb82`
(merged Task15). The dedicated branch is `codex/rewrite-16-scale-resilience`;
its PR targets `rewrite-in-rust-native-desktop`. P58 numerical requirements
retain their original ceilings. Passing workflow observers from Task15 do not
establish these scale/resource gates.

## Retained backend workload —16.1

`scale-backend` consumes all5,000 exact rows from the frozen language-neutral
performance corpus and all24 original320×400 PNGs. It preserves the64-title
cycle, chronology, UUIDs, official metadata,2,500/2,500 languages,500/500/4,000
invalid/missing/valid archives,714 Broken flags and1,666 Calm assignments.
The six actual saved Collections retain their IDs, definitions and order.
Record-only archive rows never imply usable export payloads.

The observer invokes production database-worker search and Collection-summary
APIs, including snapshot validation, count, ordering and24-row enrichment.
Every measured result must match independently derived frozen identities/order
and exact counts. Deep page200 retains offset4,776. Plans share the exact page SQL/binds with execution, including LIMIT24/OFFSET4,776;
separate COUNT plans cover each measured query and all six saved Collections.
Metadata/archive/tag index obligations are inspected,
without comparing engine-specific plan strings byte for byte.

Each query/count gate warms once then retains all20 wall samples. Nearest-rank
p95 must meet query100/search120/combined150/six-counts250/deep-page120ms.
Artwork follows the qualified fixed100 scratch method: one warmup, then100
batches performing the record lookup, reopening and reading all24 files, fresh
CRC-checked full RGB decodes
and consumption of every pixel. Compressed/output scratch is reused; decoded
results are not cached. Its independent procedural RGB sum is1,173,893,696 per
batch and wall p95 remains≤50ms. No failure is replaced by a retry/sample.
Process CPU accompanies every wall sample; artwork also records actual lookup,
read and full-decode wall/CPU phases. These explain CPU work versus scheduling or
worker wait and never replace the unchanged wall gate. Failed measurements stay
in the diagnostic record.
The pixel witness uses the original qualified probe's `black_box` and wrapping
fold over every decoded byte. At most24×320×400×3×255 fits in `u64`, so this
retains the exact independently checked sum while avoiding an extra checked
addition for each byte. Local phase isolation found that witness overhead was
only part of decode time; this correction alone does not establish an Intel pass.
The workflow uploads an early backend snapshot before starting native workloads,
including on failure. Its completeness flag and raw measurements remain decisive;
the upload itself never establishes a pass. A second complete artifact follows
all native/resource/display lanes.

`scripts/verify-native-scale.py` requires clean committed source and matching
native hardware/compiler. It retains the Cargo-reported executable/profile,
source/lock/fixture hashes, hardware, complete raw samples and actual plans.
Backend method version2 uses the production release profile: optimization3,
debug assertions and overflow checks disabled. The actual Cargo profile and
executable bind that method. Method1 inherited Task15's optimization1 checked
callback/capture observer profile; all of its retained passing and failed
cohorts keep their original verdicts. The specification defines the frozen
workloads and ceilings without requiring optimization1. The spike's packaged
release artwork probe establishes release lineage for artwork; the other SQL
and Shelf limits originate in the frozen Go qualification. This profile change
does not establish the cause of historical timing variance or numerical
acceptance. Fresh complete three-host method2 qualification is required.
All fixture rows, SQL/binds/plans/results, sample counts, full pixel consumption,
Shelf outcomes and numerical ceilings remain unchanged. Ordinary result
assertions remain active in release. The observer owns fresh isolated roots,
positively closes the database and retires the fixture. It never discovers
normal profiles or contacts production HTTP.

`Rust scale and resilience` executes the observer separately on Apple Silicon,
real Intel macOS and Linux. Reports under `target/quality/native-scale` identify
their actual hardware; the old M4 Go observation is not a cross-machine timing
comparison. Final-source three-host qualification and independent review remain
required before Task16 is checked complete. This record does not qualify
installed packages, signing, upgrade/rollback or cutover.

## Large Shelf review and bounded surfaces —16.2

The same retained executable accepts `--shelves` in a separate fresh isolated
library. It consumes `review5000.json` directly: all5,000 exact UUIDs, titles,
Broken flags and overlapping-source identities must match. One complete warmup
precedes four full preview samples; every sample and the recomputation+50-Add
commit retain the independent5,000ms ceiling. Approved revision/count/Broken
count/source count and4,950 deferred proposals equal the frozen Go outcome.

Additional actual repository workflows visit every bounded range:200 review,
creation and5,000-member detachment pages,198 exclusion pages and each affected
Shelf in linked Collection deletion. Every page holds at most25 rows; next/huge
pages are empty. Exclusion UUIDs and all final5,000 member UUIDs/order are checked
against the frozen source. Keep-all detachment retains5,000 manual members and
historical inactive origins. Linked deletion exposes one affected Shelf at a
time, with exact25/2,500 affected rows. A real SQLite trigger forces the final
delete to fail after intermediate detachment writes; transaction rollback must
preserve every member, position and4,950 exclusions. A fresh explicit plan then
successfully deletes the Collection while preserving those memberships. Exact
UUID/order reads of both Shelves and all exclusions follow each outcome. The
trigger accepts only the intended final-delete stage, after2,525 manual-retention
writes and source detachment; only a proven-not-committed SQLite constraint
receipt qualifies the rollback.

A further32-source needs-attention fixture visits both warning pages (25+7)
in update and creation previews, including next/huge empty ranges. Incomplete
coverage retains a previously approved forest member, suppresses inferred removals
and still permits an explicitly selected healthy Moon addition.

The wrapper independently checks the complete outcomes, ranges and raw timings
and seals both subprocesses to one unchanged executable/source. This is backend
and bounded-range evidence; native loading/input and resource gates remain
separate. Three-host final-source runs are required for section acceptance.

## Current native loading/input and cache observation —16.3

`scale-native` starts the current production controller, view callbacks and menu
composition against a supervisor-owned profile with exactly1,000 stories and
1,000 distinct locally decoded320×400 covers. Preparation is outside the child
launch; the supervisor remains alive until positive worker shutdown. The fixture
contains a genuinely imported archive, then exercises real1,000-input duplicate
import batches while navigating. It keeps a second real batch queued so a completed
batch does not create an unloaded gap inside a timed sample; the active/queued
count is retained. It never uses a public catalog client.

Native-input method version2 builds this workload with the production release
profile: optimization3, debug assertions and overflow checks disabled. Actual
Cargo profile fields and executable bytes bind the observation. The retained
spike also built and packaged its native-input executable with `--release`
(`task2-qualification.yml`). Task15's optimization1 checked observer was created
for callback/capture qualification; its original rationale explicitly kept
production/resource/package profiles separate. Standalone method1 inherited
that development observer choice. Its failed cohorts retain their failed
verdicts. Measuring production-release behavior does not explain the variation
between byte-identical method1 binaries, or qualify any failed startup/CPU run.

All native-input workloads, sample counts and numerical ceilings remain the
same in version2, including both actual rendering paths and positive shutdown.
Task15's checked functional suite stays separate. The paired resource workload
already used production release3; both
lanes now declare version2 in their native workload reports. Workload readiness
is a diagnostic and does not replace the separately measured startup endpoint.

Every one of42 repository pages is displayed and observed; models retain at most
24 stories (16 on the last page). A retained Arc from the first cover differs
after visiting all1,000 covers and returning, proving actual production LRU
eviction rather than merely reporting cache constants. The bounded16-command
intake/coalescing regressions remain part of the cumulative gate.

Actual CGEvents on disposable macOS CI and X11 key events on a private Linux
session measure64 key and64 pointer acknowledgements and20 completion samples after one
warmup. Input p95 stays≤100ms. Clearing a zero-result search expands the actual
1,000-story query into the production24-row page; native Tab/End then selects the
last visible story, with completion p95≤3,000ms. Before every completion
trial144 different covers evict the first page; the timed clear-search response
must advance its observed decoded-Arc generation. Thus the measured response
includes actual fresh thumbnail work, alongside the real import. Pointer samples
click the actual first-story bounds obtained from the native accessibility tree
and acknowledge the resulting browser focus. Both key and pointer p95 stay≤100ms. Every endpoint requires real
import activity and bounded models. This is the equivalent paged product
interaction, not a1,000-row artificial component model. Raw samples and endpoints
are retained; no frame/timer statistic substitutes for either wall gate.

Both default Skia GPU API and explicit software routes must identify their
actually selected surface. A Linux Mesa GPU API route does not claim physical
GPU hardware. Three-host clean-source sealing and independent review remain
pending; local development page/cache runs alone do not qualify native input.

The first full Linux run passed native typing but timed out on pointer focus in
both development and release workloads. Its retained public-tree readiness
witness is a plain label: the inspector repeats the same story title. The old
pointer picker also accepted that label. Pointer targeting now requires the
actual browser ListItem, records its role/position before
delivery and retains the first matching title's role for diagnosis. Fresh native
cohorts must establish the correction; the earlier failures remain retained.
On macOS the selected Slint mapping converts ListItem to AccessKit ListBoxOption,
then AXStaticText. The row is distinguished by its named browser-list parent and
press action, both retained with the target; filtering for AXGroup was incorrect.

## Current release resources, archive and native display —16.4

The actual production release executable is launched against a separately
prepared supervisor-owned profile. Current resource method version2 retains the
spike's startup endpoint: parent launch through delivery of the first ordinary
UI timer after show, including loader/exec and observer delivery, with the
original1000ms ceiling. Its Opening/Library phase is explicit; an Opening tick
establishes event-loop startup, not a loaded Library. Positive owned AX/CG or
AT-SPI observation of the actual first loaded story remains mandatory before
idle sampling and has its own separately named elapsed time. No product probe
flag or old spike executable supplies either observation.

Historical method1 applied the1000ms ceiling to parent launch through actual
first-story AX/AT-SPI observation. That was a different, stronger endpoint.
Its external observer initialized before launch and waited for the owned PID;
observer preparation, launch-to-PID and attach-to-story phases remain retained.
The Intel4.635s and ARM1268/2030ms receipts retain their failed method1 verdicts.
The ARM2030ms run included202 AX requests accumulating1245ms within a1979ms
attach phase. Such time may include product startup waiting and is never
subtracted. Current AX observation starts at the application's public AXWindows,
visits each window in preorder and retains the same first-loaded-story predicate.
Its bounded trace retains results, durations, traversal depth and visited counts;
serialization follows observation or appears in retained failure logs. No
historical AX-ceiling failure is retrospectively accepted by version2.

Resource and display lanes retain independent failures in one incomplete
manifest; a failed input/resource cohort does not suppress display scenarios.
One warmup and three fresh measured runs per actual rendering route retain all
raw data: first-UI-timer startup≤1000ms and idle CPU≤5% of one core over the
fixed15-second quiet phase after actual loaded-story observation. The retained
method excludes two settling seconds. Every warmup also meets all startup,
idle CPU and peak RSS ceilings; its label only separates it from the three
measured samples, matching the spike.

The native Quit driver records the owned application menu's actual title.
AppKit names it `librairii-desktop` for the directly launched production binary;
the `.app` size witness supplies `CFBundleName=Librairii`. Both exact titles are
recognized. Quit still requires the exact enabled native Quit item and AXPress,
followed by positive application/controller shutdown. The earlier title-mismatch
failure remains failed. Linux traversal skips absent accessibility children
within the same bounds and still requires the actual showing story or ListItem.

Quit method version2 visits only the owned application menu's direct items,
reading role/title/enabled together. The retained Intel trace showed version1
descending into Services and exhausting its2-second observer deadline before
visiting Quit. The new depth2 candidate witness excludes Services descendants;
the256-node bound,2-second deadline, exact enabled Quit action, and20-second
positive application/controller join remain unchanged. The earlier failed
attempt remains failed and does not establish shutdown completion.

Native time records peak RSS across each real product process and a paired
release-built current-view/controller workload containing the full1,000-cover
traversal, real imports and native input. Their larger peak must stay≤512MiB.
The paired workload includes additional observer allocations; both exact release
profiles and executables are retained. Failed runs stay in the cohort.

A measured unsigned internal archive contains the current production binary,
compiled UI/translations and current upstream notices/inventory; it must stay
≤32MiB. The observer executable and synthetic corpus do not enter this archive.
This size witness does not qualify installed distributions, signing or release
licensing; those remain Task18.

Current native frames exercise wide1180×760, short1180×400, narrow560×400 and
intermediate980×600 content with long French titles at both1×/2× requested
render densities on each renderer. Actual Winit monitor/surface and Slint scale,
logical dimensions, viewport/row geometry and capture bytes are retained. Linux
also moves the current window four times between two private headless Wayland
outputs at actual native1×/2× scales; these are virtual monitor transitions,
not a claim about physical monitor hardware. Every window/controller joins before positive library-fixture retirement; the
owned compositor joins before its separate runtime directory is retired. The workflow's larger
120-minute job envelope accommodates builds and the full repeated cohorts; no
individual numerical ceiling changes.

The short-window fixture initially exposed a real layout failure with a restored
import notice: the wide400px window left71px for a76px story row. Short main
content now caps the existing scrollable control area, preserving a full row
while keeping Search/notice actions reachable through their existing focus
reveal behavior. Actual1× release captures pass all five wide/short/narrow/
intermediate/restore cases after that fix; full current-source host sealing is
still required.

## Focused generative/fuzz and coverage gaps —16.5

A fresh APFS disk-full rehearsal returned an unexpected SQLite I/O error and
then failed to confirm image detachment. That run remains failed; its generic
SQLite error does not establish ENOSPC. The wrapper now retains its owned work
directory on every failure, records initiating and cleanup errors together, and
copies child stdout/stderr into the CI failure artifact. Only positive disposal
and a successful rehearsal permit fixture deletion. The synthetic Rust driver
records actual injection/errno and SQLite primary/extended codes, then checks
source bytes, reopening and saved authority before preserving any unexpected
error as a failed verdict. Six focused wrapper tests include a real failed child.

The retained full-workspace coverage artifact for source a0a4ba3 (run40738553488,
artifact12837679986, SHA256
`921a5e52828848f6a0a34392d9e0d77af16fb7f948fa6b2730ce58fc8801dac9`)
identified uncovered explicit rollback-acknowledgement failure and schema-receipt
validation branches. Coverage is used to choose scenarios, without a percentage
claim replacing custody/recovery assertions.

New real SQLite RAISE(ROLLBACK) cases cover both body-error and late-cancellation
paths whose subsequent rollback acknowledgement fails. They require an uncertain
outcome, fenced ordinary intake, fresh writer-barrier truth with no partial rows,
uncancelled explicit repair and successful owned reopen. Eight adversarial schema
receipts/copies cover wrong version/library identity, empty/duplicate/escaping
members, invalid checksum, wrong byte count and substituted preserved bytes.
Reinspection retains the exact native owner; confirmation must refuse without
changing any original database/WAL/SHM, archive or evidence byte or releasing
custody. Existing crash/upgrade/commit/cancel scenarios remain required.

Three256-case properties exercise ASCII saved-query stability, lossless Unicode
paged navigation and first-selected UUID decision order. A minimized Unicode
seed (`𝕬`) disproved an overstrong idempotence test assumption: frozen Go1.26.5
independently confirms Decode produces `A` and Migrate produces `a`. That exact
observable pass count is retained as a fourth regression, alongside its original
proptest seed and a fuzz input; the codec is unchanged.

The separately pinned nightly fuzz lane now also drives saved-query/reference/
migration and native URL/history/request codecs, with retained JSON, overflow,
Unicode and URL seeds. Its30-second,16KiB-input,2-second-case and512MiB bounds
match the archive/JSON lane; active corpora and crash outputs live in owned copied
working directories. Independent URL/hash and JSON frames include valid matching Library, Collection,
legacy dynamic Collection and curated Shelf history seeds. Source review caught
and corrected an earlier harness that could only reach history rejection; its
1,841,529-input diagnostic is retained without a successful-history coverage claim.
Final clean-source focused coverage/fuzz evidence remains required.

The successful-history harness exposed ASan quarantine pressure under the fixed
512MiB process ceiling. Its failed run is retained. With the same11 retained
seeds and seed1, explicitly bounding delayed-free quarantine to64MiB completed
2,857,091 executions in31s at390MiB RSS. The lane records that setting; address
checking/leak detection and all input/time/process bounds remain enabled.

Six observed test identities append to the prior622-entry baseline (628 total);
all old identities, target scopes and historical source-map digests remain exact.
The source map records the additions and its current upstream digest changes
accordingly. A current-source cumulative gate and independent review still gate
section acceptance.

## Component batch watchdog

At286199d the Intel component fixture completed its callback checks and advanced
through frame step117, retaining78 PNGs, before the old90-second process watchdog
killed scale1. That run remains failed; scale2 did not execute. This envelope
covers all callback fixtures and88 image captures per scale. It is distinct from
product startup, interaction, query and Shelf timing gates. The envelope is now
180seconds, with a raw wall-time/completion receipt on success or failure. All
176 capture, geometry, focus and callback obligations remain required. A fresh
complete run must qualify the change; no scheduler cause is inferred.

The same Intel scale run failed artwork139.199ms (process CPU139.253ms) and
deep paging127.575ms (CPU124.78ms). Pure descheduling cannot explain those
CPU-backed failures; their underlying cost remains unidentified. Native GPU
preconditioning also stalled at revision109 with two active imports. Its later
restart-required error does not establish the cause, and software did not run.
The observer now retains failure-only controller phase/status/revisions and
bounded operation metadata before shutdown can mask its triggering error. A
separate shutdown receipt distinguishes window, timer and worker close errors.
The controller also retains its initiating recovery cause in process diagnostics,
while the UI continues to show localized recovery guidance. Once
finish is requested it stops admitting further import/page work and still waits
for positive worker shutdown. No failed interaction becomes an accepted sample.
The observer also latches its first timer failure: asynchronous event-loop quit
may allow another tick, which must not offer more work or replace the first
receipt with a secondary shutdown refusal. Separate shutdown diagnostics remain
required. Earlier failed cohorts keep their original outcomes.

The46 Intel rerun reached the cold-page preload with displayed revision109 and
pending revision172 after native pointer selection callbacks. Its next page
offer hit the production intake's16-command limit. The observer now waits for
the displayed snapshot to acknowledge those earlier commands before offering a
cold-page request. This wait belongs to existing preconditioning, before the
timed clear-search response. All42 pages,144 evicting covers per trial,64 key/
64 pointer/20 cold-completion samples, original deadlines, real imports and
positive shutdown remain required. The bounded production intake is unchanged;
the earlier failed, unretired cohorts remain failed. Fresh three-host execution
must qualify this correction.

The48 ARM resource run failed its software warmup pointer p95 at128.8475ms;
all64 samples and the failed verdict remain retained despite positive shutdown.
Inspection found that each selection projection copied all24 unchanged decoded
covers into fresh native images. An owned local replay of the installed production
callback retained1,536 replacements across64 selections. The projection now reuses
an image only while UUID, decoded allocation identity and dimensions match. Its
entries are replaced by the current projected rows, cleared on library generation
change or an absent page, and hold weak decoded identities rather than extending
the worker LRU. Changed or removed covers still update immediately.

The native component regression exercises repeated selection, changed pixels,
changed dimensions, untouched neighboring covers and release of invisible entries.
The same actual-controller local replay now retains zero unchanged-image
replacements;64 callback projections took3.388ms versus16.427ms before the fix.
These are allocation/callback diagnostics on the local host, not native event or
render latency acceptance, and do not establish the sole cause of the failed VM
pointer intervals. Fresh complete three-host native/resource qualification and
independent review remain required; no sample count, workload or ceiling changes.

The a5 ARM startup trace retained1783.055ms, including1095.169ms in AX calls,
with repeated blocking window requests before a window was available. The
external observer now waits on public WindowServer metadata for its owned PID's
visible normal window before querying AX. Both waits remain inside the original
launch clock; only the actual loaded-story AX witness establishes readiness.
Window poll count and elapsed time survive success and failure. Avoiding early
AX interference is a hypothesis for the fresh run, not an established cause or
permission to discard the failed startup cohort.

The same Intel cohort exposed stale-state admission in the observer: all128
input endpoints reported five active imports, and a release workload offered57
batches while its displayed revision remained1. Import admission now requires
the displayed snapshot to acknowledge the latest controller revision before
using its active/preparing counts. The wrapper independently requires one or two
active imports at every timed endpoint. An isolated real-controller run visits
all42 pages/1,000 covers, observes two imports, joins workers and retires its
fixture. Native input and resource timings still require fresh three-host runs.

The514 ARM cohort retained1369.537ms to the actual AX story. Its owned window
appeared at1286.488ms, while AX calls themselves consumed47.106ms. That result
does not establish first-timer timing or the cause of the window delay. The
production main now emits five ordinary startup-stage diagnostics, including
exactly one first UI-timer line with PID, phase and physical size. There is no
alternate qualification startup path. The parent drains bounded output, records
actual delivery against its pre-launch clock, and joins its reader even when PID
discovery fails. Six real child-pipe tests cover delivery, identity, duplicate
markers, malformed markers and output limits before qualification begins.

The initial local release diagnostics retained Opening rather than claiming a
loaded Library: software child178.810ms/parent626.103ms and GPU API
child88.597ms/parent96.629ms. They were terminated diagnostically without joined
shutdown; both supervisor roots remain retained. They establish neither native
qualification nor fully loaded startup performance. These observations also
demonstrate why the child-relative elapsed time cannot replace parent delivery.

Resource method version2 restores the spike's defined startup endpoint:
parent launch through delivery of the first owned UI timer after show, with
the original1000ms ceiling. The spike explicitly did not measure first
presentation (`qualification/section-2/performance.md`, lines28–30;
`measure-resources.py` startup method). The first ordinary production timer
retains its actual Opening/Library phase; an Opening tick does not establish
loaded-Library readiness. Child-relative stage time is diagnostic only.
`launch_to_first_ui_timer_ms` is the gate and `launch_to_ax_story_ms` separately
retains actual loaded-story observation. Positive owned AX/CG or AT-SPI story
observation is still mandatory before the unchanged15-second idle phase.
All warmups and three measured cohorts per renderer, CPU/RSS/archive limits,
positive shutdown and original samples remain required. Earlier AX-ceiling
failures retain their original method and outcomes; they are not recomputed as
passes. Fresh complete version2 cohorts and independent review are required.

The514 Intel native interaction cohorts passed both rendering routes, but its
separate idle product did not exit within20 seconds after a posted CG quit
shortcut. That failed sample and unretired fixture remain retained; the receipt
does not identify whether event delivery or controller shutdown was responsible.
Resource shutdown now invokes the owned application's enabled native Quit menu
item through AXPress and retains its PID, actual menu item, supported action and
result. Deadline checks occur during traversal and before activation. Ordinary
production diagnostics distinguish a shutdown request, event-loop return and
controller join. The20-second positive-exit requirement is unchanged.
Bounded helper stdout/stderr and status survive nonzero exits and timeouts;
two additional real-process checks verify that retention. All eight resource
preflight tests pass locally; fresh actual native menu execution is required.

At a5ec473 a fresh query-codec fuzz run retained a75-byte malformed-UTF8 seed
that violated the harness's Unicode idempotence assertion. Its15-byte Cherokee
reduction also reproduces locally. Actual frozen Go1.26.5 Decode/Migrate/Encode
independently produces the same alternating Cherokee forms. Both inputs are now
tracked regression seeds, and the existing Unicode regression checks the exact
observable passes. The fuzz target compares the migrated wire with its owned
query's payload and the results of two entry points that each add exactly one
canonical pass, including bounded rejection. It no longer assumes arbitrary
Unicode idempotence. Production codec behavior and ASCII stability checks remain
unchanged. The failed cohort stays retained; fresh complete fuzz runs are required.
