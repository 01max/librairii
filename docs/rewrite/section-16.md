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
The workflow uploads an early backend snapshot before starting native workloads,
including on failure. Its completeness flag and raw measurements remain decisive;
the upload itself never establishes a pass. A second complete artifact follows
all native/resource/display lanes.

`scripts/verify-native-scale.py` requires clean committed source and matching
native hardware/compiler. It retains the Cargo-reported executable/profile,
source/lock/fixture hashes, hardware, complete raw samples and actual plans.
The observer uses the qualified opt-level1 profile with assertions/overflow
checks, owns fresh isolated roots and positively closes the database and retires
the fixture. It never discovers normal profiles or contacts production HTTP.

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
prepared supervisor-owned profile. Public native AX/AT-SPI observation of its
first loaded story establishes readiness, including process launch, loader,
profile/database opening, controller/view construction and observer delivery.
No product probe flag or old spike executable supplies that measurement.
The external AX/AT-SPI observer initializes before the product launch clock and
waits for the owned PID over a pipe. The clock still starts before spawning the
production executable and stops only after its actual first-story observation.
Observer preparation, launch-to-PID and attach-to-story phases are retained
separately. The first Intel receipt's4.635s observation remains failed; fresh
phase measurements must distinguish observer startup from product startup.
Mac readiness reads title/description/children/role in a single public AX request
per node, retaining request count and time. This removes redundant observer IPC
while preserving the full production launch-to-loaded-story interval. The later
ARM1268ms receipt remains failed until a fresh complete cohort meets the ceiling.
The subsequent ARM2030ms receipt also failed:202 public AX requests accumulated
1245ms of wall time inside a1979ms attach phase. That time may include product
startup waiting and is never subtracted. The observer now starts at the owned
application's public AXWindows and visits each window in preorder, avoiding
native menu branches while preserving the exact first-loaded-story predicate.
A bounded in-memory trace retains AX results, durations, traversal depth and
visited counts; serialization follows observation (or appears in retained failure
logs). Fresh complete cohorts must establish the effect; old failures remain.
Resource and display lanes retain independent failures in one incomplete manifest;
a failed input/resource cohort does not suppress the remaining display scenarios.
One warmup and three fresh measured runs on each actual rendering route retain
all startup/idle data:≤1,000ms readiness and≤5% of one core over a fixed15-second
quiet phase (two settling seconds excluded by the retained method).
Every warmup also meets all startup, idle CPU and peak RSS ceilings; its label
only separates it from the three measured samples, matching the retained spike.

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
