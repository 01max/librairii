# QUAL2.10-392-COUNT-BUDGET

Qualification-only derivative on PR38, starting at39267cffd281223cb9295f276cf6f7c041fcc701
with admitted4383373272ce1d3095199caf37dde7e5d0598213 ancestry. No product decoder,
dependency, process architecture or refusal policy is selected.

The authenticated sevenz-rust2=0.24.1 derivative checks declared file/coder counts
and `usize` conversion / checked `count * size_of::<T>()` **before the two target
vector allocations** in reader.rs. Experimental ceilings are10,000 files,64 coders
per block and8MiB requested bytes per targeted vector. The file-count number
coincides with a frozen application limit; this placement/implementation and the
coder/vector ceilings are experimental, not established product compatibility.
The structured `QualificationCountBudget` error has no heap payload and is distinct
from decoder malformed-input errors, process signals, timeout or OOM.

This does not bound earlier header allocation, aggregate blocks/counts, names,
properties, dictionaries, all other allocations or CPU. It leaves
`MAX_MEM_LIMIT_KB` unchanged. The two retained negatives are synthetic declarations
with padded headers, not proofs of accepting every valid10,000-entry archive.
All general memory/dictionary, constructor CPU, global multiblock/empty-entry stop,
solid preceding work, full/partial CRC and long-Read cancellation gates stay open.

## Source identity and derivative recipe

`upstream-source.json` inventories every79 regular member of the authenticated
1,020,812-byte published `.crate`, SHA256
`2f0c68c0e8289bfd09e49ef433e8de02c7c464afc1d7f5a275c99749a880e301`.
Source: https://static.crates.io/crates/sevenz-rust2/sevenz-rust2-0.24.1.crate
VCS:ab17c9b9184df49941ffcb0df4f41cb06d106ef8; upstream MSRV1.93; Apache-2.0.
The exact LICENSE and original headers are retained. This is an isolated source
experiment; no product-license assignment or root inventory change follows.

The selection is all `src/**` plus LICENSE, README.md, Cargo.toml, Cargo.toml.orig
and .cargo_vcs_info.json:32 original files, with original hashes/first12 lines.
No upstream examples, tests, binary payloads or expanded cache tree is copied.
`count-budget.patch` changes error.rs/lib.rs/reader.rs and adds the guard module;
the resulting33 files have an exact inventory. Replay into a NEW temporary tree
and verify both replay and shipped derivative (including no extras/symlinks):

```sh
python3 -B qualification/section-2/decoder-budget/source_recipe.py \
  --archive ABS_CACHE/sevenz-rust2-0.24.1.crate
```

The only child-lock change is the pinned registry decoder becoming this local
source derivative. All52 other registry versions/checksums are unchanged.
Defaults remain disabled, decoder features bzip2/ppmd/deflate/zstd/lz4/brotli;
no new helper/dependency is added. The child is its own workspace and excludes the
vendored upstream package from workspace testing. Root Cargo/CI remains untouched.

## Execution boundary

Only `../count_budget_stage1.py` is the Stage1 entrypoint. Do NOT invoke the old
32-case supervisor main for this stage. Stage1 reuses its explicit runtime guards,
five actual controls, per-container HostConfig validation, kernel readback and
kill/wait/inspect/removal. Controls must pass in the same invocation before any
fixture or hostile constructor dispatch. Optimized-interpreter regressions mock
runtime boundaries, not the actual controls. ELF must be LinuxARM64. There is no
host fallback and no overwrite of historical output.

The only fixtures are files-count-allocation (2,097,184 bytes),
coders-count-allocation-larger (8,388,640 bytes), and solid-ordered (214 bytes).
Their exact retained SHA256s are in the entrypoint. Historical213-byte source-read
observations for the solid input are not its214-byte full input length.
All run with128MiB cgroup/no swap, AS256MiB, CPU2s, wall5s; the deliberate wall
control uses2s and CPU control8s. A killed harness is never an explicit decoder
refusal or passing guard result. Each case records boot/before-constructor/callback
markers, structured result, actual memory.events/peak, exit/signal and elapsed.

Build only from this nested cwd with explicit1.99.1 or the retained pinned official
LinuxARM Rust image. Use a fresh Cargo cache seeded ONLY with the52 lock-verified
registry archives/index, letting Cargo extract fresh sources, and owned target/output
mounts. The observed build command and result belong to new Stage1 evidence; a
build attempt or old392 approval does not supply runtime proof.

```sh
# After an actual successful current-source LinuxARM build AND source verification:
AO_SESSION_ID=librairii-4 python3 -B qualification/section-2/count_budget_stage1.py \
  --binary ABS_OWNED_TARGET/release/qualification-decoder-budget \
  --output NEW_OWNED_PATH/count-budget-observations.json
```

No hostile archive may run on the macOS host. Safe scalar/arithmetic Rust tests
run from this directory using `rustup run 1.99.1 cargo nextest`, explicit root
nextest config/profile ci, locked target; prior two assertions remain alongside
two new boundary/overflow tests. This binary-only child has no library doctest
harness; do not claim zero-example documentation execution as a passing lane.
