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
