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

First implementation commit SHA will be recorded in a subsequent scoped ledger
commit (a commit cannot contain its own content hash). Dedicated task trailer:
`OpenSpec-Task: 5.1`.

## Verification status

Module-local tests are authored but **NOT EXECUTED**: owner5 must first add
the actual core export/discovery/assertions from the first published exact SHA.
Formatting/source review is not a test pass. Final combined exact-head checks,
native review and provider CI remain pending. No full5.N or5.10 completion.

Future real admission still requires enforceable pre-constructor resource
strategy, complete format/codec/CRC/failure/cancel proof, real storage and
canonical hooks plus separately released effect paths. Owner4's six filenames
are independent producer/reader string observations, not Go decoder goldens or
captured raw7z name bytes. No fixture, baseline, shared root or Go17 change.
