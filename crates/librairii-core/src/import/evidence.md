# PURE5 partial implementation ledger

Release: PURE5-4ea2caf. Exact admitted base: `4ea2caf71bbe61ed996942daa9005de05b75f9da`.
Frozen source: `73a93303a9a44cf0a92eaf525331561ed8c04460`.
Additional bounded release: PURE5.4-M438, from prior validated candidate
`b3cfb5cd966e587ad789cc1464fe34203818ef18` with admitted repair
`4383373272ce1d3095199caf37dde7e5d0598213` retained in history.

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
full checks remain separate. Original partial5.3 commit:
`2dba142d12066e8d47d35243e6ceb2076a6e1a47`
(`feat(import): preserve supplied pack identities and structure`,
`OpenSpec-Task: 5.3`).

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

## Partial5.8: orthogonal reported facts/errors only

Report data retains frozen3 outcome/9 importer error labels, optional raw UUID/
checksum/ID fields and first-inspection-error traversal through nested/joined
causes. Absent and present-empty cause/data remain distinct. Captured outcome
and commit observations cannot be overwritten by record_problem; independent
primary/stage-cleanup/compensation/report problems retain order and owned data.
These internal observations are NOT storage acknowledgements, receipt types,
persisted operation states, runtime ports, authority or a new schema.

Five added tests bring scoped execution to22 passed (10 unrelated tests filtered):
late cancel/cleanup/report errors retain committed facts/IDs; uncertain errors
invent neither IDs nor noncommit; duplicate/conflict observations retain raw
fields despite independent errors; join traversal preserves first validation
order and optional empty values. This is value-policy behavior, NOT real source/
copy/hash/art/SQL/compensation fault injection, race, restart or cancellation
execution. No row/file/lease/guard exists in this slice.

Future effects require actual owner6-issued capabilities/revalidation and safe
guarded unresolved-effect successor transfer. Proven committed receipt reporting
can transfer without holding custody exclusion through every report retry;
needed generation/resources remain owned until actual runtime drain. No API
implementing that protocol is supplied here.

Original partial5.8 commit:
`c5ef496d6203a11024b9a4075dbf0280ec9f01cd`
(`feat(import): retain orthogonal reported facts and errors`,
`OpenSpec-Task: 5.8`). All full5.N/5.10 remain open. Final union discovery
must cover every actual new shipping test and all prior admitted IDs; the two
independent temporary suffix comparisons are not shipping baseline entries.

### Separate partial5.8 correction: structured validation causes

The initial validation cause field could retain only a string, losing nested or
joined causes allowed by frozen inspection/contracts.go:61–81 and importer
service.go:244–277. ValidationError now owns an optional boxed FailureCause;
present-empty messages remain distinct from absence. A separate regression
retains all joined children, checks standard Error::source traversal and confirms
the outer/first validation error wins without disclosing raw path bytes through
Display. The standard single-source interface does not flatten joined children;
the owned tree retains them. This is a separate corrective commit, not a change
to shared errors or an effect/error port. Scoped nextest now passes23 tests
(10 unrelated tests explicitly filtered); all-target core Clippy-Dwarnings and
scoped rustfmt pass. No final combined mandatory-suite claim follows.

## Original implementation/support SHA index

| Partial task / support | Original full SHA |
|---|---|
|5.1 supplied-fact dispatch/codes | `6ec300c6a5dfadfd295198d08618ad3ae5920c79` |
|5-owned exact-SHA export/discovery support | `daef6c5d8d4ed8cc056a059630b7da2cc72f34a8` |
|5.2 supplied entry/path/duplicate/accounting | `53dbf76711c5f7b53fa1d462573dfd023569fe49` |
|5.3 raw/typed UUID and required entries | `2dba142d12066e8d47d35243e6ceb2076a6e1a47` |
|5.8 reported facts and nested/independent errors | `c5ef496d6203a11024b9a4075dbf0280ec9f01cd` |
|5.4 PARTIAL supplied STUdio UUID/references/thumbnail presence | `5218d65c88696dc21211782508f1f32e6f7503a3` |

All five implementation commits are Conventional Commits with their own
OpenSpec-Task trailer. The required ledger-recording commit does not contain
its own self-referential content hash. No implementation SHA is squashed,
rebased away or replaced by later review/support/evidence commits.

## Partial5.4: supplied STUdio stage/reference/thumbnail policy

`studio.rs` maps frozen studio_zip.go:19–29/64–90/108–147 and the exact-key
`has` behavior in zip_reader.go:60–94. Borrowed decoded UUID/image/audio strings
and ordered nodes are supplied facts, not parsed JSON. `ImportIdentity` preserves
raw accepted UUID spelling and import-local nil rejection; errors get story.json
context without fabricating google/uuid parse-cause text, which remains unmodeled.

After validating limits, policy order is UUID, each node's IMAGE then AUDIO,
then regular thumbnail.png presence. Skip only empty strings; zero nodes or
globally zero nonempty references give missing_asset(stageNodes). Repeated refs
and empty individual nodes are allowed. A boolean tracks nonempty presence;
there is no invented stage count, deduplication or asset extension restriction.

Path failures preserve outer unsafe_path with its nested path error. A valid
path outside the case-sensitive cleaned assets/ prefix has no invented inner
cause. Critically, cleaned paths are for safety ONLY: lookup uses the ORIGINAL
reference unchanged against Inventory's cleaned-key index. Thus an alias such
as assets/./cover.png is safe but missing_asset even if assets/cover.png exists.
Directories do not satisfy referenced files or required thumbnail presence.

Eight new regressions cover raw UUID forms, image/audio/node/thumbnail error
priority, empty/global reference rules, repetitions, nested versus prefix causes,
raw dot/slash aliases, distinct Unicode forms, directory versus file and missing
thumbnail, exact512-byte/16-depth edges, and10001 repeated supplied nodes over
only3 inventory entries. These are synthetic source-derived policy cases, not
newly generated goldens or actual archive/JSON/PNG observations. Inventory fields
and identity output certify no actual bytes, decoding, inspection or custody.

All31 import tests passed in the already-wired namespace, with10 unrelated
core tests explicitly filtered; scoped fmt and all-target core Clippy-Dwarnings
passed on1.99.1. This is a scoped run, not a final mandatory-suite result. The
eight new IDs still require owner5's fresh actual UNION discovery support on the
first published scoped SHA; all47 prior IDs/scopes and original supports remain
unchanged. No shared root, dependency, manifest, testkit or preservation edit.

JSON wire/type/presence/null/duplicate/trailing behavior, actual metadata/artwork
read caps, image parsing/bytes, archive I/O/CRC/decoder/constructor/resource/cancel
and all runtime/storage/SQL/custody remain UNRELEASED. Missing story.json still
belongs to the existing dispatch/pack fallback, not a new STUdio missing-story
rule. Title/description trimming is outside this narrow supplied-node policy.
Full5.4 and5.10/all full5.N boxes remain OPEN. No new-head review or independent
108-input pass is claimed from integration's preparation-only working inputs.

## Prior complete candidate and current remaining gates

Original cause correction: `31ca0adc101d10ad8c9f203e730e97f68eb79276`.
History-preserving actual repair merge: `2b9a455849cb107a98c02bcf9e3afd38c7955d80`.
Original final discovery support: `b3cfb5cd966e587ad789cc1464fe34203818ef18`.
All initial implementation/support SHAs above remain ancestors, not replacements.

On EXACT priorb3 (before this5.4 addition), own full native ARM xtask passed47
tests/0 skips plus2 separate compile-fail docs,146 admitted corpus/strict6 Python,
workspace+fuzz inventory/deny/audit/fmt/build/Clippy. Full make check build passed
including Go/frontend release207 tests/responsive/visual/accessibility/performance
and native Wails build. The initial settings-cleanup5s timeout remains distinct
from subsequent unchanged focused4-case and full-chain passes. All8 exact-b3
provider checks succeeded, last2026-10-20T18:59:58Z. Single AO native8ddbc420
approved with no required findings (GitHub5589684509 COMMENTED, not independent
account approval). This evidence qualifies ONLY the previous partial scope.

Read/hash-verified integration2 artifact `section-05-pure-policy-integration-verification.md` SHA256 `2c2d353c3c04717931a8d12a5a765b23369c6753f2d637ac00190993c1525456`
records5 temporary tests/666 bounded comparisons plus49 fact states/193 late-error
invariants on exactb3; not our reproduction, exhaustive behavior, new shipping IDs
or5.4 verification. The older2.2M suffix proof remains exactdaef only.

Current scoped source results:31 import tests passed,10 unrelated tests
explicitly filtered, all-target core Clippy-Dwarnings passed and scoped
rustfmt passed on1.99.1. Full mandatory runs must have no filtered/skipped
required tests. Source-free startup PATH failures are not code/test results.

Final NEW-HEAD readiness is PENDING: then-current admitted global ancestry;
5-owned fresh UNION discovery for all31 import tests plus all prior assertions;
final combined
locked build/fmt/Clippy/nextest+separate doctests/146fixture+strictPython/both
supply-chain graphs; cumulative make check/frontend release/build and provider
Go/Linux/native/focused CI; independent affected reference-policy comparison and
ONE fresh native review of the exact ready head. Neither priorb3/daef results nor
prepared108 inputs supplies these later-head gates. All full5.1–5.10 remain open;
no full task completion or
merge authority is claimed.
