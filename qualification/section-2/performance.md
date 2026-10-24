# Automated resource measurement continuation (2.9, 2026-10-23)

Task 2.9 is complete at `e9ab1ba` / native run 70, independently reviewed. Artwork
method v3 reuses compressed/output scratch with a fresh full CRC-checked decode
for every actual read. Run 68 remains failed; its mixed CPU/wall cause is unknown.
See [the retained failure and experiment](completion-evidence/artwork-scratch/).

All three native targets pass the 64-event input, 20-trial expansion/navigation
and 100-batch artwork gates, together with all warmup/measured resource ceilings.
Exact samples and source/package identities are in completion-evidence/final-matrix.json.
Input ≤100 ms p95, navigation ≤3000 ms p95 and 24 local 320×400 full PNG
reads/decodes ≤50 ms p95 remain unchanged. Additional ceilings are startup marker
≤1000 ms, idle ≤5% of one core, peak RSS ≤512 MiB and archive ≤32 MiB. Startup
markers are not first physical presentation; three resource runs are reported as
min/median/max, not a fabricated p95. See [selection-gate.md](selection-gate.md).

Earlier observations below retain their dates, source commits and historical scope.

`measure-resources.py` measures the verified archive-round-trip executable on
its matching native host. CI requests normal `winit-skia` and explicit
`winit-skia-software`; actual selected surfaces are recorded rather than inferred
from the request. Each backend gets one complete warmup, then three fresh
process measurements. Raw runs, min/median/max, source/executable hashes, CPU and
memory identity, scale/window size and session variables are retained in
`resources/resource-measurements.json`. Disk caches are not flushed. These small
samples are diagnostics, not a new p95 distribution or approved budget.

`--resource-probe` emits its PID on the first owned UI timer after `show`.
Parent launch-to-marker includes process/loader startup and observer delivery;
it does **not** measure first visible presentation. The probe keeps the normal
16 ms UI polling timer during a fifteen-second quiet phase. No component input or
synthetic background job is injected then; initially visible PNG prefetch can
settle. The sampler excludes the first two seconds and the last half-second,
requires at least 20 samples spanning six seconds, and discards sampler calls
taking more than 100 ms. The reported CPU percentage is a cumulative process CPU
delta over that interval, with 100% meaning one core. Linux reads proc stat ticks
and RSS pages; macOS reads `ps` accumulated user/system time and RSS (1024-byte
units). Sampling the observer/time wrapper as though it were the app is avoided
by the app's PID marker and owned process-group check.

After quiet sampling, the same bounded 1,000-PNG thumbnail/component-input probe
runs under synthetic background work and normal owned cancellation/join/fixture
retirement. `/usr/bin/time` captures full-run wall/user/system time and maximum
RSS through that workload: macOS reports RSS bytes; GNU time `%M` uses
[KiB-sized memory units](https://www.gnu.org/software/time/manual/html_node/Memory-Resources.html)
which the recorder converts to bytes. The existing thumbnail probe is shared,
not a second decoder implementation. Its first-24 diagnostic in the resource
run includes already-prefetched visible images and is not a cold artwork budget.
Native time output and each CPU/RSS sample remain available for review.

Each process group is bounded to 40 seconds, including quiet time and the
existing 20-second thumbnail deadline. Timeouts, signals, missing metrics,
insufficient idle coverage, unowned PIDs, missing surface identity, wrong explicit
CPU surfaces, incomplete workload and missing owned shutdown fail measurement.
A failed warmup or measured run is retained without replacing it with a retry.
Reports are saved after each run. Package payload bytes include all verified
manifest members plus the manifest. Archive matching streams and verifies every
regular payload member's size/digest and exact manifest bytes, rejecting missing,
duplicate, changed, unexpected or alias members; macOS DMG size/hash records
the supplied builder output without remounting it here.

The first three-target release measurements passed in
[run 39961452717](https://github.com/01max/librairii/actions/runs/39961452717),
implementation `b6016f1`, built source `ecbdcb7b9fea00455dd7c33baf6a32605925aefa`.
[resource-observations.json](resource-observations.json) retains exact source,
binary/distribution identities, actual hardware/surfaces and all six summaries;
the linked artifacts retain the raw 24 runs (six warmups and 18 measurements).
Independent review recomputed every verdict/summary successfully. Those retained
results used the historical ten-second quiet phase. The current fifteen-second
phase adds observation margin after Intel run40020211745 correctly rejected an
18-of20 low-latency sample set; that failed raw trace stays failed. Sample count,
six-second coverage,100ms collection guard and40-second deadline are unchanged.
See [native-linux.md](native-linux.md) for the retained failure and correction;
the methods are distinct and no product-performance budget is approved.

These are within-host diagnostic observations, not comparisons with the older
physical-host baseline. Explicit Skia CPU medians were:

| Identified CI host | Readiness marker | Idle CPU (one core) | Peak RSS | tar.gz bytes |
| --- | ---: | ---: | ---: | ---: |
| ARM, Apple M1 (Virtual), 3 CPUs | 301.7 ms | 1.50% | 273,907,712 | 10,676,944 |
| Intel i7-8700B, 4 CPUs | 301.9 ms | 0.98% | 216,297,472 | 11,215,538 |
| Linux EPYC 9V74, 4 CPUs, Xvfb | 48.9 ms | 0.41% | 74,080,256 | 14,615,761 |

Task 2.9 remains incomplete: physical input/first presentation, representative
product-artwork timing and additional evidence-based ceilings remain open.
No existing budget is weakened, passed or replaced by these resource diagnostics.

The same head's separate cumulative Rust check retained a Linux failure in the
inherited job progress test: 537/538 passed, then `controlled observation` timed
out at ten seconds. A temporary 100 ms delay in fetching its second item
reproduced the exact failure with only one executor active, one item prefetched
and the coordinator blocked by the deliberately held progress callback. Waiting
for both executors before releasing their existing gate passes that same
schedule without changing the deadline, progress load or assertions. The
temporary delay/debug instrumentation was removed; only fixture coordination
changes. Production progress/coordinator policy remains unchanged.

At the corrected fixture head `0a174f2`, all three native qualification jobs
passed again in [run 39963960217](https://github.com/01max/librairii/actions/runs/39963960217).
Linux's cumulative run passed all 538 tests, and ARM also passed. The separate
Intel cumulative run retained another Shelf budget failure in
[run 39963960567](https://github.com/01max/librairii/actions/runs/39963960567):
535/536 tests passed; worst preview was 5.371917264 s against the unchanged
5 s ceiling, while applying 50 decisions took 3.934265908 s.

Independent investigation found that the debug-mode wall-clock measurement
shares nextest slots with storage, hashing and crash fixtures, each spawning
additional workers. Archived Intel execution shows overlap with 44 other tests.
The CI profile now reserves all runner slots for this exact retained-budget test
(`threads-required = "num-test-threads"`, supported by pinned nextest 0.9.148).
No test is skipped/retried, the workload/assertions and five-second ceilings are
unchanged, and other tests still run. Contention is a likely explanation rather
than a proven product root cause; new full native CI must verify the isolated
measurement. If it still fails, the preview path needs profiling and correction.

The first full native execution with that scheduling passed all three Rust
targets at implementation `53aa12f` in
[run 39971403712](https://github.com/01max/librairii/actions/runs/39971403712):
Linux 538/538, Intel 536/536, zero skipped. Intel's retained Shelf test passes
its unchanged five-second assertions. Its reported 18.416 s is the **whole
fixture duration**, including setup and all additional checks, not preview
latency. This supplies a passing isolated measurement without erasing either
earlier Intel failure or proving that repeated-row metadata hydration is cheap.

# Thumbnail measurement probe (2026-10-23)

The current [continuation](continuation.md) replaces shared/generated rasters
with a lazy original-PNG worker and bounded LRU. Run the packaged executable
with `--thumbnail-smoke` (normal backend and explicit CPU path) and preserve its
source manifest and complete output. It runs 1,000 decodes under background work,
reports cold first-24 completion, a bounded last-512 PNG decode p95, component key
dispatch p95/sample count and peak shared-cache entries. The outer smoke runner
kills a hang at 30 seconds; the workload itself fails after 20 seconds.

This fixed 48×64 synthetic corpus is useful regression evidence. Component
callback timing excludes OS input delivery and presentation; cold first-24 is a
single queue/event-loop sample and one-PNG p95 is not the batch-of-24 budget.
No retained ceiling or additional hardware budget is passed by this smoke alone.
[Operator checks](operator-checks.md) still require physical measurements,
startup/presentation, sustained idle CPU, RSS, package size and representative
artwork across actual hosts/renderers. The observations below are historical.

# Resource diagnostics (2.9, P58)

**Partial; no new performance ceiling approved.** 2026-10-20, librairii-4.
Host: macOS 15.7.7 M4 Pro / 48 GiB, CLT SDK26.2; Rust1.99.1.
Release GUI graph: Slint1.18.1/Winit0.30.13/Skia0.153.3, 1,000 shared-artwork
synthetic stories. Source: a5d38d1 plus pending dev-only data-probe dependencies;
no feature UI, real archive import or distinct thumbnail decoder workload.

One `/usr/bin/time -lp` run of `SLINT_BACKEND=winit-skia-software
qualification-desktop --smoke` exited 0:

| Measurement | Observed diagnostic |
| --- | ---: |
| Model/window construction marker | 99 ms |
| Wall (includes deliberate 3 s timer) | 3.39 s |
| User / system CPU | 0.08 / 0.05 s |
| Maximum resident set size | 299,237,376 bytes |
| Peak memory footprint | 131,056,624 bytes |
| Raw optimized Mach-O size | 25,155,000 bytes |

Binary SHA-256:
`25ec42d47bf2bbcd95e785c55c43e0127cb960715c7d4ff8156f59737363877a`.
This is neither a package nor final distributed-byte checksum. Initial build
completed with rust-objcopy debug-stripping warnings about its libLLVM lookup
when Cargo was invoked without rustup's environment. Subsequent commands use
`rustup run` with the pinned toolchain bin on PATH. The runtime smoke succeeded;
the toolchain warning must be distinguished from a measured UI regression.

This run does not measure time-to-visible, sustained idle CPU, input p95, distinct
thumbnail decode/eviction, background saturation, or GPU failure. Native input
access is denied; other target hosts are missing. Do not interpret the three-second
timer as passing the ≤3,000 ms expansion gate. Do not compare these host diagnostics
with old-host timings or invent p95 from one sample.

Retained mandatory ceilings: 5,000-story query100/search120/combined150/six-counts250/
deep-page120 ms p95, 24 artwork loads+full PNG decode50 ms p95; 5,000-story Shelf
preview/apply5 s; native1,000-story completion≤3,000 ms and real input dispatch
p95≤100 ms. Review/warning/create/exclusion ranges≤25 rows, one affected Shelf/page.

For each renderer/target, warm once and capture enough identified samples, native
input timings, startup/idle/RSS and final package sizes during actual thumbnail
load and cancellable background work. Separate scheduler pauses from product work.
Set additional budgets from those measurements before full feature UI rollout.

## Retained artwork workload continuation

`--artwork-probe` materializes the exact24 Task1 procedural320×400 PNGs in an
owned temporary directory before measurement. One complete batch warms the
local files/decoder, then20 measured batches each read all24 files and fully
decode/consume every pixel. Nearest-rank p95 (19th sorted sample) must be≤50ms;
all samples and the accumulated pixel witness are retained. The actual packaged
release executable runs this on each native target. Fixture creation, encoding
and cleanup are outside the measured interval; fixture retirement is mandatory.
This is the retained load/full-decode budget, separate from synthetic48×64
thumbnail-queue and OS-input observations. Fresh native measurements are pending.

## Additional initial regression ceilings

The identified-host measurements above support additional startup-marker≤1000ms,
sustained idleCPU≤5% ofone core, peakRSS≤512MiB and verified tar archive≤32MiB.
These round ceilings leave roughly2–3× margin over the recorded maxima (normal
andCPU backends, ARM/Intel/Linux). They are initial spike regression ceilings,
not claims of p95 from three runs or physical first-presentation measurements.
Every warmup and measured run must satisfy them; failures remain in the report
without sample substitution. Actual native keyboard100ms p95, expansion/usable
navigation3000ms and retained24-artwork50ms p95 remain separate unchanged gates.


## Fixed100 artwork method (2026-10-24)

Run40118957740 Intel preserves a failed20-sample wall-p95 observation:
72.528676ms against50ms. Two adjacent samples are125.260460/72.528676ms;
the remaining18 span17.894621–35.377489ms. The exact pixel witness remains
24651767616. This is a failed measurement, not a scheduler diagnosis or retry.

Method read-full-decode-v2-fixed100 predeclares one warmup and100 measured
batches on every native target. Each batch performs the same24 frozen320×400
local reads, complete PNG decodes and consumption of every pixel. All100 wall
samples remain; nearest-rank p95 is sorted[94] and the unchanged ceiling is50ms.
CPU time from CLOCK_PROCESS_CPUTIME_ID is recorded separately per batch to help
distinguish decode cost from stalls; it never replaces the wall-time gate.
The already selected libc0.2.191 supplies that public process clock; no dependency
version/source/feature was upgraded. Independent review approves the measurement
refinement, with fresh execution required. The original20-sample failure is not
rescored. The101-iteration pixel witness includes the single declared warmup.

## Linux native input measurement boundary (v3)

Run40120658865 retains a private X11 input64 p95 failure154.590690ms against
100ms. Its previous measurement included two foreground-inspection xprop
processes before native posting; macOS checks foreground ownership before its
clock. V3 checks the exact foreground PID immediately before starting the clock,
then includes the complete injector process/post and actual edited-state20ms
rendezvous. All64 wall samples, the1000ms per-event deadline,100ms p95 ceiling
and active-workload guards remain. Foreground, posting, acknowledgment and total
including foreground are retained separately; no diagnostic replaces the gate.
The original failure is not rescored and host contention is not a proven cause.
A delayed-preflight regression rejects the former boundary; slow native posting
still produces a failed gate after the correction. Fresh native execution is
required before qualification.

Run40122322247 preserves a distinct private Wayland v3 failure148.690334ms;
X11 passes49.431676ms and both offline-VM routes pass. Posting remains near9ms,
while four actual acknowledgements stall140–239ms. Wayland callbacks generated
16ms apart are dispatched233ms apart and then catch up, localizing the stall to
the client without proving computation, recorder I/O or scheduler cause.
Bounded read-only diagnostics now retain the owned main thread's CPU/runtime,
runqueue and kernel wait state during slow acknowledgements, plus cgroup CPU/I/O
facts. They change no scheduling setting, workload, sample count or gate.
