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
