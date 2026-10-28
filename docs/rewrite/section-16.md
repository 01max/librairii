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
import batches while navigating. It never uses a public catalog client.

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

## Current release resources, archive and native display —16.4

The actual production release executable is launched against a separately
prepared supervisor-owned profile. Public native AX/AT-SPI observation of its
first loaded story establishes readiness, including process launch, loader,
profile/database opening, controller/view construction and observer delivery.
No product probe flag or old spike executable supplies that measurement.
One warmup and three fresh measured runs on each actual rendering route retain
all startup/idle data:≤1,000ms readiness and≤5% of one core over a fixed15-second
quiet phase (two settling seconds excluded by the retained method).

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
