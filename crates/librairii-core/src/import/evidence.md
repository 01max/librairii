# PURE5 partial implementation ledger

Release: PURE5-4ea2caf. Exact admitted base: `4ea2caf71bbe61ed996942daa9005de05b75f9da`.
Frozen source: `73a93303a9a44cf0a92eaf525331561ed8c04460`.

Only effect-free `core/src/import/**` is released. All full5.1–5.10 remain
open. No inspection/decoder/classifier/CRC execution, storage/custody receipt,
runtime/cancel/lifecycle port, image/JSON parser or effect is implemented here.
Policy counters/plans and reported fields cannot establish actual decoder
bounds/integrity, commitment or cleanup authority.

## Initial partial5.1

Frozen `internal/catalog/repository.go:12–22`, inspection contracts, ZIP/7z
dispatch source and tests supply the seven format labels, sixteen validation
codes, suffix/content precedence and ZIP-full versus7z-selected integrity
requirements. `ContentFacts` are caller-supplied observations; a successful
`ArchivePlan` is only a plan. Original filename bytes are never rewritten.
ASCII suffix matching additionally retains Go Unicode15 simple-lower mappings
U+0130→i and U+212A→k; it performs no search folding or Unicode composition.

Original partial5.1 commit: `6ec300c6a5dfadfd295198d08618ad3ae5920c79`
(`feat(import): add pure format dispatch policy`, `OpenSpec-Task: 5.1`).

Original owner5 additive wiring: `daef6c5d8d4ed8cc056a059630b7da2cc72f34a8`,
direct parent6ec300c6; consumed by fast-forward. Its only changes are core lib.rs
public import and four actual required test IDs; prior24 IDs remain retained.

## Verification status

Initial6ec300c6 tests were NOT EXECUTED before wiring. After consuming original
daef6c5d, the working-tree partial5.2 Rust sources passed scoped nextest:11 passed,
10 existing tests filtered by `import::` (not a full-suite or mandatory-skip waiver).
Command: `cargo nextest run --locked -p librairii-core import::`, using exact
Rust1.99.1 toolchain bin and existing `/private/tmp/librairii-section3-tools/bin`
on command-local PATH. Initial PATH-only startup failures executed no tests.
Scoped rustfmt also passed. Final combined exact-head full discovery/checks,
separate doctests, native review and provider CI remain pending; no full5.N/5.10
completion or transferred ancestor check result.

Future real admission still requires enforceable pre-constructor resource
strategy, complete format/codec/CRC/failure/cancel proof, real storage and
canonical hooks plus separately released effect paths. Owner4's six filenames
are independent producer/reader string observations, not Go decoder goldens or
captured raw7z name bytes. No fixture, baseline, shared root or Go17 change.

## Partial5.2: supplied entry/name/size policy

Maps frozen inspection limits.go, zip_reader.go and sevenzip_reader.go.
All21 retained archive-header boundary inputs are consumed via include_str, with
exact expected codes; no decoder or fixture generator executes. Additional
source-grounded tests cover raw-invalid UTF8/NUL/backslash/absolute/drive names,
Go lexical dot/parent aliases, terminal duplicates, directories versus files,
link/negative-size error priority, nested suffixes and distinct ZIP/7z ratios.
NFC/NFD are separate synthetic strings and are not composed or relabeled Go
archive observations.

Unsigned sizes above i64 and checked compressed/expanded/count overflow refuse
without wrapping. Failed Budget updates are atomic; inventory failure is terminal
so a rejected partial inventory cannot be presented as complete. These are pure
caller-data properties, not actual decoder allocation/read/CPU/CRC enforcement.
Original partial5.2 commit: `53dbf76711c5f7b53fa1d462573dfd023569fe49`
(`feat(import): add bounded entry accounting policy`, `OpenSpec-Task: 5.2`).

## Partial5.3: supplied UUID and required-entry facts

`pack.rs` ports pure required-entry ordering and raw-root precedence from frozen
zip_inspector.go; UUID parsing matches pinned google/uuid v1.6.0 Parse forms
(32/36/38/45 bytes) plus import-local nil rejection. In the documented legacy
38-byte form only the middle36 bytes are examined; delimiter bytes are retained,
not silently hardened. Shared StoryUuid parsing/BoundaryError are unchanged.

Tests distinguish nil/malformed/text/binary UUID, raw spelling from canonical
diagnostic text, two raw roots with equal typed UUID, missing files/assets in
source order, and absent versus present-empty supplied metadata. CandidateData
is supplied data only: no JSON/image/container is interpreted or CRC/custody
authority established. Six additional tests bring scoped execution to17 passed
(10 unrelated tests filtered). All-target core Clippy with -Dwarnings passed
after fixing two local style diagnostics; scoped rustfmt passed. Final combined
full checks remain separate. The next scoped ledger update records this
original commit SHA. Trailer: `OpenSpec-Task: 5.3`.

Integration independently verified suffix behavior on EXACT wireddaef6c5d,
not later commits:1,112,064 actual frozen-Go1.26.5 scalars and2,224,128 public Rust
comparisons plus seven actual filename-helper observations;2 temporary tests
passed with0 skips/filters. Read/hash-verified nonshipping integration artifact:
/Users/maxime/.ao/data/artifacts/librairii-2/section-05-pure-integration-verification.md
SHA256 `fef686bff0d24cc4b0bb3b750651c2f362a17b480ce34ca5a8100812fccd943b`.
This is independent suffix-policy evidence, not our reproduction, new shipping
tests, raw7z names or later-head/UUID/decoder/resource/custody qualification.

Root reports a separate admitted foundation special-path preservation-wrapper
defect; owner5 exclusively repairs it on its own support PR. Ordinary-path
passes do not qualify Unicode/space/#/%2F checkout behavior. No duplicate repro
or root fix here. After reviewed admission, consume actual new global history,
owner5 union-discovery support and rerun combined final-head checks/review.
