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
The test fixture captures its actual native directory entry spelling before
closed verification, and compares the retained source name with that spelling.
It does not impose NFC/NFD or treat filename strings as raw7z header evidence.

## 5.8 PARTIAL: unused-stage retirement and physically retained failure

After any work error/cancel, the only mutation is storage's existing
`retire_owned_temp(&custody)` without ambient cancellation. Success returns the
actual RetiredTemp owner and its recoverable trash identity; it is not deletion,
import compensation, restart cleanup or a clean-finalizer receipt. Failure
returns the actual boxed TempCleanupFailure, including stage FD/directory and
any trash witness/cleanup uncertainty/cause. Source and guard remain in the same
outcome. The primary work error, independent cancellation observation and cleanup
cause coexist. No Result-only error discards actual owners; Drop never cleans.

Fault wrappers are private and borrow actual OwnedReadSource/OwnedStage objects.
They inject errors/short operations/EOF/cancel without minting origin, a digest
or cleanup authority. Tests assert real prefix bytes, source bytes and retained
exclusion. A failed Write may have physically written bytes that it never
acknowledged; the counter explicitly does not claim that physical length.

The inner sentinel regression intentionally supplies a smaller defensive bound
to the private copy loop over an actual larger managed source. The public path
always derives its expected size from the real initial observation; a managed
source that changes is normally refused by the issuer's own metadata checks.
No adversarial interleaving/immutability proof is inferred from that regression.

The retirement-failure test substitutes the actual private stage container
before retirement, checks both actual old FD bytes and foreign named bytes, and
retains the real pre-move failure/guard. This does NOT dynamically exercise a
post-rename directory-flush uncertainty. Such a failure is retained faithfully
by the same real TempCleanupFailure type, but actual G fault evidence for that
branch and restart/guarded successor resolution remains open. Issuer allocation,
stage allocation and arbitrary blocked-read interruption are still excluded.

The SAME source/stage/guard relationship is a caller-held host precondition,
established by real constructors in the tests. The current storage API has no
public non-consuming stage/guard identity-check operation; this mechanism must
not be advertised as a general admission API proving arbitrary inputs share an
activation. No shared production port or capability conversion is added.

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

## Original contribution ledger

| Partial contribution | Original full SHA |
|---|---|
|5.6 successful-stage-input copy/hash|93894aeeed9b425403f11dc8fac7645998c00781|
|5.8 uncancelled unused-temp retirement/retained failure|02d7e4762f3cf3f9322ce3160f11ccbf5ea7e5d3|

The ledger update is a separate documentation commit. No commit is a5.10
completion/admission, review or executed-test receipt. All originals and the
entire662 ancestry are retained without squash/rebase/cherry-pick.

## Authored candidate test identities — NOT observed discovery

These17 source declarations exist under cfg macOS/Linux. The expected parent
module path is `archive::custody::tests`; owner5 must derive the actual nextest
identities AFTER wiring, rather than pinning a declared count. Two additional
compile-fail examples on CopyOutcome are authored, NOT executed doctests. Existing
parent assertions/scopes/docs and both source-policy closures remain unchanged
in firstG. Tests use existing librairii-testkit::TestRoots, so the prospective
owner5 support needs that dev-only internal edge as well as adapters->storage.
No serde/JSON/crypto/decoder or other external dependency is requested.

- `archive::custody::tests::prepublication_copy_retains_real_native_owners_raw_name_and_exact_eof_bytes`
- `archive::custody::tests::prepublication_empty_source_requires_actual_eof_and_empty_stage_digest`
- `archive::custody::tests::prepublication_growth_sentinel_is_read_but_never_written_past_bound`
- `archive::custody::tests::prepublication_nonempty_stage_is_refused_without_appending_source`
- `archive::custody::tests::prepublication_qualification_envelope_is_not_a_product_or_expanded_limit`
- `archive::custody::tests::prepublication_short_copy_eof_retires_only_actual_owned_prefix`
- `archive::custody::tests::prepublication_real_borrowed_read_error_preserves_prefix_and_uncancelled_retirement`
- `archive::custody::tests::prepublication_unacknowledged_write_prefix_and_cancel_never_erase_io_cause`
- `archive::custody::tests::prepublication_short_writes_and_interrupted_read_still_measure_real_eof`
- `archive::custody::tests::prepublication_acknowledged_partial_write_error_keeps_exact_bytes`
- `archive::custody::tests::prepublication_cancellation_phase_matrix_retains_owners_without_partial_digest`
- `archive::custody::tests::prepublication_cancellation_between_read_and_write_or_short_writes_is_observed`
- `archive::custody::tests::prepublication_hash_read_error_or_short_eof_produces_no_digest`
- `archive::custody::tests::prepublication_changed_source_or_actual_stage_is_never_measured_success`
- `archive::custody::tests::prepublication_retirement_failure_keeps_actual_stage_source_guard_and_foreign_bytes`
- `archive::custody::tests::prepublication_already_owned_work_can_finish_after_ordinary_intake_stop`
- `archive::custody::tests::prepublication_checked_counter_overflow_preserves_prior_observation`

## G2 PARTIAL5.8: actual retained stage-creation result routing

Release `IMPORT-G2-RETAINED-CREATE-e0c` starts a NEW source sibling from original
Gca330. It does not change shipping44=662, original G, C_G479 or fixedS_G44.
The selected new API input is EXACT
`feda9132a20817f8e31882a24d28a228f383c186` (original stage failure fix
`e0cde94e62f5f3db79a0dc211d8008df2add1567` plus frontend7ba5). Explicit future
generic discovery support input is ORIGINAL
`954934f44ed29b78e0bf220a749319a682681730`, directfeda. These pins are NOT merged
into the source-only G2 branch. Owner5 owns a NEW distinct composition preserving
both ORIGINAL C_G479 and B954 histories, their source closures and parent test
objects/order/scopes. No parent export/graph/dependency/source-policy edit by7.

Local copy_created_stage consumes the actual
Result<OwnedStage, Box<StageCreateFailure>>, original readonly source and SAME
Custody in the caller-held Files/DB/Native/profile frame. Ok delegates the OLD
copy_to_stage algorithm unchanged and stores its whole outcome as CopyAttempt;
that name does not imply successful copying, accepted inspection or commit.
Err moves the WHOLE original box into CreationFailure, with original source and
guard. Failure/source fields precede guard in field-drop order. The structured
cause/disposition/allocation remain borrowed through the storage API; no Error,
From, bare-cause/string conversion or fake owner constructor is introduced.
Cancellation is an independent observation. No partial digest, retirement,
cleanup, retry, fence clearance or claimed source admission occurs on creation
failure. NoAllocation concerns THIS call only and settles no earlier owner or
uncertainty. RetainedUnresolved remains physically held by the actual caller.

The positive existing issued fixture now binds create_stage's result, asserts
successful fixture setup BEFORE destructuring, and never uses borrowed-error
`?`/From. An unexpected creation failure is a failed test assertion, not a
production error route that claims resolution. Two existing borrow examples
bind the real result and pass it through the new wrapper; no conversion-related
E0277 is needed. Intended failure diagnostics are E0502 (mutable DB close) and
E0505 (Files move while outcome lives). Two paired no_run compile-pass examples
exercise the actual public result shape and borrowed cause reporting. Their
compilation/diagnostics are NOT executed or qualified on source-only G2; target
annotations alone are not claimed as a captured diagnostic pass.

Three new authored tests use real Native constructors/managed-source issuance:
successful creation delegates exact bytes and retains source/guard; successful
creation plus work cancellation delegates the old uncancelled retirement; real
NoAllocation after ordinary intake stops retains the identical Box address,
original source and guard, without copying, erasing the cause or touching an
earlier owned stage. Cancellation true/false and a deliberately refusing copy
envelope distinguish factory failure from copy-policy execution. Prior-stage
bytes and source bytes remain actual observations, not substitute capabilities.

There is NO manufactured or locally executed RetainedUnresolved allocation
case. Its actual post-mutation failpoints/optional FDs/identities/fence and native
finalization cases are generic owner6 tests, attributed separately. The public
wrapper has no private storage checkpoint or fake failure constructor. The same
Err arm retains every supplied actual failure box, but the NoAllocation case
must not be presented as dynamically exercising retained partial allocation or
an earlier native fence. Partial allocation resolution stays unimplemented and
unqualified. No ignored/pending test is used as a pass surrogate.

G2 remains UNWIRED/UNCOMPILED: only scoped Rust1.99.1 rustfmt and Git diff checks
are run before publication. No nextest discovery/test/compile-pass/diagnostic,
cargo, Go, provider/native review or current combined quality claim. The three
candidate additions below are source declarations only, not an observed UNION:

- `archive::custody::tests::prepublication_creation_success_delegates_copy_with_exact_bytes_and_source`
- `archive::custody::tests::prepublication_creation_noallocation_retains_whole_box_prior_stage_and_cancel_truth`
- `archive::custody::tests::prepublication_creation_success_with_cancel_keeps_existing_retirement_contract`

All17 original G assertion declarations remain present. Owner5 derives actual
discovery after exact wiring/serialized lane. No new root/port/pool/runtime,
allocation-failure cleanup, publication/SQL/PNG/CRC/decoder/resource/accepted
inspection, recovery/settlement/nativeClosed or full5.N/5.10 claim is made.
