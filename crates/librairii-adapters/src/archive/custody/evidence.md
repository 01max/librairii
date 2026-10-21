# Partial import prepublication mechanism — source-only G

Release: `IMPORT-G-SUCCESS-STAGE-44c`. Base is original
`662962938f66ead219fefec6a297e10943e81bb1`; fixed storage API composition input is
`44c2416ab5d79c5513dd039f02e3f866972b8067`, including original readonly issuer
`34dc8e0ab2691f7bda0b49f8bb0fe65746da019a`. Shipping PR44 remains at662.

## 5.6 PARTIAL: actual managed source to successful owned stage

One synchronous caller owns NativeProfileStore, a genuinely verified independent
copy, NativeLibrary, SAME Database and LibraryFiles. This module takes the actual
managed Archives/Catalog OwnedReadSource, an already-successfully-created empty
OwnedStage and the SAME Custody guard. Every returned outcome retains the actual
source FD/parent/name/lease borrow, stage and guard. No Path/File/TestRoots/DTO
constructor, app port, static worker bridge or general picker is added.

Copy uses at most storage HASH_BUFFER_BYTES per chunk, checked counters and one
growth sentinel that is never written past the observed bound. Observed initial
size bounds this cooperative copy; optional QualificationEnvelope is explicitly
test qualification only. Neither is a product source cutoff or the frozen2GiB
expanded budget. Source is rewound on the SAME FD, hashed through EOF before and
after copying using storage hash_stream; actual stage EOF is measured using
OwnedStage::measure. Length/digest/metadata disagreement refuses measured success.
No partial checksum is returned on any error/cancel. Acknowledged written bytes
are a lower bound if a Write error may have written unacknowledged bytes; the
actual stage, not a counter, retains the physical prefix.

Readonly size/mtime/ctime/identity/hash equality is cooperative observation, NOT
an adversarial immutable snapshot or exclusion. Cancellation is checked between
chunks/phases/short writes and by actual storage measurements; arbitrary blocked
Read is not interrupted. Error causes remain structured and independent from
the cancellation observation. Issuer construction and create_stage partial
allocation/open failure are explicitly outside this successful-input boundary.

Tests use owned synthetic source bytes placed in an independently copied frozen
closed library BEFORE actual ClosedLibraryInput::verify/copy_to_new_root and
Native binding. TestRoots only allocates owned paths. Test-only path reads assert
bytes; they never construct capability authority. Corpora/generators and normal
profiles are untouched. Teardown preserves stage/trash bytes; it does not claim
clean Native finalization.

## Verification state

UNWIRED / NOT COMPILED / NO TEST DISCOVERY OR EXECUTION CLAIM. FirstG deliberately
does not edit parent exports/manifests/lock/graph/policies. Owner5 will compose a
distinct branch FROM the published originalG, history-merge EXACT fixedS_G,
resolve its six shared conflicts, add the NEW adapters->storage edge/policy and
parent exports, and derive actual additive test discovery preserving BOTH parent
source-policy closures/notices/baselines. N3 retains serialized build priority.

No accepted inspection, format/classifier, archive read/CRC/PNG/decoder/resource
strategy, publication/reuse, SQL/commit, published compensation, durable recovery,
clean finalizer, other native target or full5.N/5.10 admission/merge is claimed.
All full section5 checkboxes remain open; no shared OpenSpec progress was edited.
