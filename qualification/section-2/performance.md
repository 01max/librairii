# Automated resource measurement continuation (2.9, 2026-10-23)

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
16 ms UI polling timer during a ten-second quiet phase. No component input or
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
manifest members plus the manifest. Archive size/hash is associated by checking
its embedded manifest against the measured package; macOS DMG size/hash records
the supplied builder output without remounting it here.

Corrected three-target execution/results are pending this revision's CI.
Task 2.9 remains incomplete: physical input/first presentation, representative
product-artwork timing and additional evidence-based ceilings remain open.
No existing budget is weakened, passed or replaced by these resource diagnostics.

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
