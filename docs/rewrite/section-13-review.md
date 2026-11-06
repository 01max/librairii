# Task 13 independent review

Review base: `44ea386f7b84ae2861aa77aaea9956dddeb7c6fa` (integration after PR63).
Two independent read-only reviewers applied the code-review skill's Standards
and Spec axes to the full section diff and rechecked every required correction.
Final source review includes `7ae68973a34a5bbfe7f180eb7cbf55efb1e7201d`.
Neither reviewer edited source, ran builds or delegated execution.

## Standards

Final report: **0 required findings**.

The review required retaining actual owners through partial journal/workspace
allocation, bounded marker reads, physical alias comparison and acknowledged
idempotent source retirement. Corrections preserve expected prefixes only
through retained same-file witnesses and admit restart journals only after
actual root/namespace/lease proof. Retirement acknowledgment precedes final
control/root removal; newly changed files remain preserved.

Two final P2 evidence findings were corrected separately: execution now uses
Cargo's reported compiler artifact with pre/post executable and driver/example
hashes, and a nonempty selected mount root fails the mandatory exact-root
rehearsal instead of returning success-shaped evidence. The final driver change
bounds retry of macOS image-detach error 16, permits forced disposal only of the
fresh private wrapper-owned image and publishes evidence only after disposal.
Other disposal errors remain failures. Product relocation never unmounts a
user volume.

The final source-policy review confirms every prior Rust/Python identity retains
its order and fields; additions match their declared positions. Historical
source/index/codec/derivative pins and checker logic remain unchanged. Only the
current additive source-map digest is repinned.

## Spec

Final report: **0 required source findings**.

A final concurrency audit found that draining a mutation and then moving allowed
both racing requests to succeed. `7ae6897` introduces distinct mutation admission:
active edits cause immediate Busy with the original generation, later edits
refuse before effects, and readers/artwork still drain. Job/DB idle checks run
with intake closed before and after drain. The real Collection regression was
observed failing before the correction and passing afterward in both orderings,
including actual relocation/reopen of the winning edit.

Corrections preserve the first system-language preference/frozen historical
fallback, actual destination DB readiness before authority commitment, explicit
publication uncertainty, and verified destination qualification before source
cleanup. Missing source without acknowledged retirement remains pending.
Executable Retry/Cancel and retention of pending copies permit later moves.

Fresh startup now suggests a native root that confirmation accepts, safely
prepares entirely absent native namespace/data parents and preserves invalid
state. Selecting the actual current root through an alias is an unchanged
operation. Final precommit revalidation checks both complete trees and the
actual record/lease. Same-UUID locate refuses foreign root controls before
SQLite mutation and prevents fallback to the stale precommit source. Explicit
root recovery works with the unused data container and its parent absent,
without recreating them, while keeping legacy separation and language.

The reviewer found no additional unasked scope. Native menu/view/focus/toolkit
composition remains Tasks 14–15; this section qualifies their executable service
boundary rather than claiming a completed Rust desktop application.

## Execution evidence

Source review does not substitute for native execution. The immutable 691f08d
macOS ARM, macOS Intel and Linux checkpoint receipts are retained in
`section-13-native-evidence.json`; they cover cumulative discovery and executed
JUnit identities, the frozen-Go isolation rehearsal, eight actual process kills,
real EACCES/ENOSPC, a separate filesystem, exact selected mount-root publication,
DB reopen and positive owned-volume disposal. Each receipt identifies
the actual provider merge source and its implementation-head/base parents. The
7ae6897 mutation-admission fix has its own complete local gate. Final pushed-head
platform execution/source/artifact receipts are recorded on PR64 and the local
canonical handover before readiness; the older checkpoint cannot replace them.

Earlier failed/partial checkpoints remain historical. In particular, the first
complete ARM rehearsal passed its product assertions but failed image disposal;
it is not counted as a passing provider job. The corrected final source requires
all three jobs and the required-target aggregate to pass.

Required findings remaining: Standards **0**; Spec **0**. Native/provider
acceptance and later GUI/package/release gates remain distinct.
