# Decoder resource experiment (2.10 / P07)

**Unqualified. No archive admission or production isolation strategy is selected.**
Observed2026-10-20 by librairii-4: a qualification child in LinuxARM64 Docker
Desktop on the ARM Mac in hosts.md. This is not Linux x64 desktop, native macOS,
Windows, assistive-tech or clean-package evidence. The experiment confirms a
pre-callback exhaustion path; a container bounds this experiment only.

`decoder-observations.json` contains all five controls and32 synthetic cases,
including every input byte count/SHA256, original producer/source hashes,
ELF hash, callback markers, actual reads, elapsed time, inferred signal from exit
status, kernel memory counters and available peak measurements. Input generators
are original `synthetic_7z.py` / `adversarial_7z.py`; no product library/profile,
upstream fixture bytes or classifier is used. Host Python3.14.7 links liblzma5.8.4
(observed `_lzma` linkage via otool and `lzma_version_string`), zlib runtime recorded
in JSON. Deterministic generators reproduce hostile bytes without parsing them;
large zero padding is generated locally rather than embedded in Git. Do not feed
these inputs to an uncontained decoder.

## Boundary and controls actually observed

The owned child and its constructor run entirely inside the boundary. No host
decoder parses a hostile header. The outer Python supervisor creates one labeled
container per case, checks effective configuration and removes only its own ID.
There is no network, writable root, credential/home/socket mount, extraction,
publication, cleanup authority or production path. Fixtures and ELF are read-only.

* cgroup v2 `memory.max`134217728 bytes, `memory.swap.max`0; Docker memory and
  combined memory/swap both128MiB. `memory.events` is captured before decoding and
  by a surviving shell after child exit; the wall watchdog captures it before kill.
* Kernel soft/hard `RLIMIT_AS`268435456 bytes and `RLIMIT_CPU`2 seconds read back
  through `/proc/self/limits`. One CPU quota,32 PIDs, no capabilities,
  no-new-privileges,8MiB temporary filesystem and core size0 also apply.
* Parent wall limit5 seconds per decoder case. The wall control deliberately uses
 2 seconds; CPU control uses8 seconds so CPU enforcement can be distinguished.
  Kill/wait/inspect/removal have separate bounded CLI deadlines. These are observed
  limits for this harness, not frozen application budgets or guaranteed timing on
  another host. The wall timer begins at container start, after creation.
* Controls run before **any** hostile fixture parsing in each invocation. Benign
  smoke succeeds;512MiB reservation aborts under AS;192MiB touched allocation
  triggers `oom_kill=1`, exit137; wall sleep is killed by the parent; CPU spin is
  killed under CPU2s without the wall watchdog or OOM. The shell records post-child
  counters, including an exact134217728-byte peak for the resident-memory control.
* Benign `solid-ordered` runs under identical limits: exit0, two callbacks, ordered
  names/lengths/CRCs match, `oom=oom_kill=0`. `copy-read-baseline` also succeeds.

The original Docker CLI refused an `as` ulimit option. No hostile probe ran under
that failed startup. The final launcher sets AS through Bash before child start,
then verifies kernel readback. The five-control prerequisite also has a regression:
failure records a tooling error and prevents the experiment function entirely.
Toolchain/startup/control failures are not decoder verdicts.

References for interpreting measured counters, not substitutes for measurements:
[kernel cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html),
[Linux resource-limit API](https://man7.org/linux/man-pages/man2/getrlimit.2.html),
[Docker memory/swap options](https://docs.docker.com/engine/containers/resource_constraints/).

## Observations and refusal categories

| Case | Actual observation | Limit of inference |
| --- | --- | --- |
| `files-count-allocation` | Original2097184-byte input, SHA256 `1de99665b09c5a6209ad222a6be49825ef56cfa319246131a53b4c4f9a6b13f0`;2M file count in2MiB header; boot only, zero callbacks, SIGKILL/exit137, `oom=1`, `oom_kill=1`,128MiB peak | **Decoder resource exhaustion before callback.** OOM is not an implemented product refusal. |
| `coders-count-allocation-larger` | Original8388640-byte input, SHA256 `5acc6aa505a5f0aab9c6ed6437ce2ce86b999491ff5ba1ac6ef9769d5daf1ccb`;8M coder count in8MiB header;512000000-byte reservation fails, SIGABRT/exit134, zero callbacks, no OOM event | Address-space allocation refusal terminates the child; no graceful product error. |
| u64-max header size/offset and file count | Decoder header errors, no callbacks | These specific malformed declarations reject; not proof for every header/count. |
|10,000/10,001 entries | Metadata-only allow/refuse at frozen threshold | Header parsing/allocation already occurred. No full10,000-entry decode measured. |
|2GiB /2GiB+1 /u64-max declared output | Metadata-only allow/refuse; overflow covered separately by Rust checked-sum test |11MiB trailing input padding isolates whole-container ratio; not actual multi-GiB output or CRC qualification. |
| Ratios200/201 | Metadata-only boundary allow/refuse | Uses aggregate declared size / entire container length, preserving frozen Go denominator. |
| Real512KiB repeated payload | Declared-ratio guard refuses before callback; diagnostic raw mode reaches65537 bytes and returns read-budget error | The64KiB delivery probe has a one-byte detection sentinel. It is not an exact64KiB allocation cap or a2GiB expansion proof. |
| LZMA2 dictionaries8MiB/12MiB |8MiB decodes;12MiB refused by experimental metadata guard | The8MiB limit is **not** frozen policy; rejecting these otherwise valid inputs requires explicit compatibility review. |
| Large LZMA1/2 dictionary declarations | Tiny payloads decode; LZMA2 with declared2GiB also returns success after540 bytes, recorded as `decoder_eof_with_size_mismatch`; LZMA1 large-size variant returns `dist overflow` | No general dictionary ceiling. Short output is retained as a diagnostic mismatch, never acceptance. |
| Solid full decode / callback cancel | Two callbacks vs one; ordered CRCs match; both read213 source bytes | Cancellation did not reduce compressed source reads here. No constructor/long-Read interruptibility or responsiveness claim. |
| Solid packed corruption | Decoder/read error retained | Specific corrupt stream only; required/unrelated payload policy still absent. |
| Before-header cancel | Zero source bytes, zero callbacks | Only caller cancellation before construction is established. |
| Source-read ceiling / delivered-byte ceiling | Structured qualification read-budget errors, distinguished from decoder rejection | Neither controls header/coder allocation nor all implicit preceding-solid work. Seeking does not reset actual-source accounting (regression). |

Five initial allocation hypotheses were **not met** and remain explicitly in the
JSON as expected/actual differences: the smaller2M-coder header returns EOF, both
tiny large-dictionary inputs decode, LZMA2 large-size input ends short, and LZMA1
large-size input returns `dist overflow`. The harness exits2 for those exploratory
differences. They are not five frozen-Go compatibility failures, passing safety
tests or reasons to invent an exception. Measured SQL/Unicode differences remain
separate in `corpus-observations.json`.

## Exact source findings and unselected production strategy

The candidate sources are pinned by both Cargo.lock files. All53 registry packages
in the child match parent spike versions/checksums; no new dependency was selected.
The child only depends directly on sevenz-rust2=0.24.1 (defaults off; BZip2/PPMd/
Deflate/Zstd/LZ4/Brotli decoder features) and serde_json=1.0.154. Source references:

* [reader.rs0.24.1](https://docs.rs/crate/sevenz-rust2/0.24.1/source/src/reader.rs):
  `MAX_MEM_LIMIT_KB` line18; `init_archive` lines342–410 parses/decodes headers before
  callbacks; file vector line522 and coder reservation line1036 happen before
  application limits. `bounded_count` lines1149–1157 uses header-byte length,
  which does not prevent the observed vector amplification. Single-thread control
  and callback `Read::take` are not allocation/work controls.
* [decoder.rs0.24.1](https://docs.rs/crate/sevenz-rust2/0.24.1/source/src/decoder.rs):
  LZMA1/2 construction uses coder properties; LZMA2/PPMd's effective allowance is
  the near-unbounded constant from reader.rs. No public ArchiveReader memory budget
  was found. An adapter that checks archive metadata afterwards is too late.
* [lzma-rust2 0.23.0 LzDecoder](https://docs.rs/crate/lzma-rust2/0.23.0/source/src/lz/lz_decoder.rs):
  `new`/`ensure_capacity`/`grow_slow` lines39–102 grow the window with actual output.
  [LZMA1 reader](https://docs.rs/crate/lzma-rust2/0.23.0/source/src/lzma_reader.rs)
  also reduces history for small known output. These source findings explain why
  tiny dictionary probes are insufficient; they do not establish measured bounds.

Frozen Go P07 remains:10,000 entries,2GiB expanded, ratio200,512-byte paths/depth16,
metadata1MiB/artwork10MiB, from oracle73a9330 `internal/inspection/limits.go` and
admitted `preservation/corpus/limits/`. The sketch only checks entry/expanded/ratio
metadata and experimental dictionaries; it is no product classifier. CPU2s,
AS256MiB, worker128MiB and dictionary8MiB are experimental, not new product budgets.

Before any affected archive admission, section5 must obtain a demonstrated,
deployment-compatible boundary **before constructor invocation**. Viable work to
evaluate is an in-process decoder with fallible bounded header/count/dictionary
allocation, aggregate work/actual-read accounting and proven interruption, or an
explicitly approved native worker/process strategy with memory/time limits applied
before parsing. A worker would need owned-stage read/seek only, stage-aware
progress and separate refusal/resource-exhaustion/timeout/cancel/decode-error
outcomes; it must have no publication or cleanup authority. Container availability
cannot be a product prerequisite or an implicit architecture change: the current
research requires in-process libraries, so process isolation would need an explicit
architecture decision and native deployment qualification first. Linux resource
limits alone were observed here within Docker, not deployed or qualified on the
four desktop targets. No such production boundary is implemented by this spike.

Absent that proof, the candidate remains **non-admitting/unqualified** for these
inputs. There is no new product refusal code, hardening waiver or compatibility
exception. Other core/storage sections can proceed independently. All physical,
AT, licensing, packaging, classifier and remaining codec/resource gates stay open.

## Reproduce safely

Existing Docker Desktop is required for **this experiment only**, no paid resources.
Pinned official builder/runtime image:
`rust@sha256:e12566d5abc17a0da741628782cdb504b68b8961f5bfcf677936bf3e8b32de29`
(`rust:1.99.1-bookworm`). Observed builder Rust1.99.1
`58cf8f9c35b355199727627d07ae34a4e2846d67`, Cargo1.99.1 `5f94df478`,
host aarch64-unknown-linux-gnu, LLVM23.1.1. Runtime Engine28.2.2/kernel6.10.14-linuxkit.

Build only the trusted child, using a fresh temporary target directory; never run
its allocation modes on the host. Substitute absolute paths for the two mounts:

```sh
docker run --rm --label ao.session="$AO_SESSION_ID" \
  --memory=1536m --memory-swap=1536m --cpus=2 --pids-limit=128 \
  --cap-drop=ALL --security-opt=no-new-privileges \
  --mount type=bind,src=ABSOLUTE/qualification/section-2/decoder-budget,dst=/probe-src,readonly \
  --mount type=bind,src=ABSOLUTE_TEMP_TARGET,dst=/probe-target \
  --workdir /probe-src \
  rust@sha256:e12566d5abc17a0da741628782cdb504b68b8961f5bfcf677936bf3e8b32de29 \
  env CARGO_TARGET_DIR=/probe-target rustup run 1.99.1 cargo build --release --locked
python3 -B qualification/section-2/decoder_containment.py \
  --binary ABSOLUTE_TEMP_TARGET/release/qualification-decoder-budget \
  --output /tmp/decoder-observations.json
```

The supervisor validates ELF, configuration and five controls first and never falls
back to uncontained execution. Expected current exit2 retains the five hypotheses
above. Missing Docker/controls/toolchain is a tooling failure, not toolkit failure.

Current-base verification after merging4ea2caf (merge48fc786): root `cargo xtask
check` passes on aarch64-apple-darwin, retaining24 nextest cases, two separate
doctests, admitted-fixture/discovery checks, locked build, fmt, warnings-denied
Clippy and root/fuzz supply-chain checks. The spike retains7/7 nextest cases in
each GUI/no-default configuration; separate doctest harnesses each contain0;
all-target GUI Clippy and both workspaces' fmt checks pass. The child retains2/2
nextest cases and warnings-denied all-target Clippy. Its binary-only package has
no library doctest target (`cargo test --doc` reports that limitation); no passing
child doctest claim is made. All seven Python tests pass, including fail-closed
supervision and independent Zstd regeneration. Debug/release controller and actual
widget Space→Apply→unchecked-twice smoke routes pass without an OS window/input.
Release logs retain the previously observed rust-objcopy/libLLVM strip warning;
processes exit0. These checks do not fill any physical/native-package gate.
