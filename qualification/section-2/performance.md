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
