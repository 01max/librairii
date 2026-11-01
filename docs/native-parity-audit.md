# Audit native parity evidence

Task 19.1 compares the sixty frozen parity contracts with implemented assertions,
actual test execution and native observations. It also identifies what still
needs final release review. The audit never authorizes signing, publication,
cutover or retirement of the Go application.

## Inputs and retained contracts

- [`preservation/parity-register.json`](../preservation/parity-register.json)
  remains the original planning register, including its historical unverified
  statuses. Do not replace those statuses with a blanket Rust pass.
- [`section-19-parity.json`](rewrite/section-19-parity.json) retains the earlier
  implementation index and the original candidate references.
- [`section-19-parity-audit.json`](rewrite/section-19-parity-audit.json) adds all
  sixty row obligations, named assertion sources and their SHA-256 values,
  accountable owner, native callback/workflow links, scope decisions and release
  holds. Its 201 case links select 175 distinct Rust tests. A test can support
  several contracts without becoming several independently executed tests.

The [semantic findings record](rewrite/section-19-parity-findings.md) describes
the three omissions found by independent review and the dedicated regressions
needed to resolve them. Older passing execution cannot supply those new cases.

The assertion summaries describe the relevant checks, not a claim that test
names prove every sentence in a contract. Read the actual assertions and raw
observations. In particular, a passed callback group is evidence for that
component fixture; it does not prove every workflow in the installed application.

## Run the offline observation check

First retain and independently select the twelve original provider ZIPs and their
metadata using the [evidence input procedure](native-release-evidence.md).
Use the same input plan for the custody manifest and the parity observations:

```sh
python3 -I -B scripts/assemble-native-evidence.py \
  --input /absolute/evidence/input.json \
  --output /absolute/evidence/manifest.json
python3 -I -B scripts/audit-native-parity.py \
  --input /absolute/evidence/input.json \
  --output /absolute/evidence/parity.json
```

Each output must be a new file beside the input plan. Existing files are never
overwritten. Keep the input, outputs, original ZIPs, failed attempts and review
receipts together. The checkout must contain the observed source commit's Git
objects; the tool reads those objects without fetching or checking them out.

The parity command is an offline reader. It does not extract or execute archived
binaries, open the GUI, mount disk images, contact GitHub, sign packages or change
the planning register. It checks:

1. All sixty IDs, original register/index fingerprints, accountable ownership,
   explicit scope decisions and release holds.
2. The observed source commit/tree and the exact reviewed implementation and
   test-file bytes, read from that source's Git objects.
3. Three distinct target families, candidate source/profile/graph, inner package
   hashes, actual retained installed bytes and the selected archive hashes.
4. Native nextest discovery and JUnit agree exactly, all cumulative required
   cases executed, and no failure, error, skip, ignored or filtered case appears.
   Every mapped case must occur in those passing results.
5. Both native scale-1/scale-2 component reports are sealed, their mapped callback
   fields say `passed`, and the fresh/upgraded installed lane reports, checks and
   retained files match their hashes. Linux requires both X11 and Wayland lanes.
6. Scale report source/target/lock identity and declared completion. Actual
   numerical results still need the independent numerical review below.

The output records the original source, target, inner package and provider
archive selections separately. It always sets `provider_authenticated`,
`semantic_coverage_verified`, `numerical_acceptance_verified` and
`public_release_qualified` to false. Those flags describe the command's limits;
passing the command is not release approval. This version accepts unsigned
internal candidates only. Final signed Mac evidence uses the separate
[signed-lineage manifest procedure](native-release-evidence.md).

## Reconcile the observations independently

For each row, compare the retained behavior, reviewed implementation, actual
assertions and reported target observations. Check edge cases rather than
inferring coverage from a source filename or test count. Keep any omission open
until a behavior test or an explicitly authorized product decision resolves it.

Review the following raw scopes in addition to the named-case output:

| Scope | Raw evidence to inspect | What the named-case output does not establish |
| --- | --- | --- |
| P01–P04, P11, P49, P52, P54–P55, P60 | Quality `storage-handover.json`, `upgrade-handover.json`, `native-relocation.json`, retained helper/compiler inputs, actual failure and forced-termination traces | Lifetime ownership, all recovery transitions, real disk-full/permissions/second-volume behavior and source/candidate separation |
| P13–P38, P43, P48, P50, P53, P56 | Quality `native-ui/manifest.json`, sealed scale reports and PNGs; installed `gui/qualification.json`, lane reports and native observer records | Readability, focus/keyboard behavior, complete assertion semantics and actual OS picker/reveal attribution |
| P56 | Linux quality `workflow-speech/` reports, utterances, acknowledgements and captured PCM/WAV; retained independent review | Spoken workflow, cancellation/silence and reader retirement; no inference of human intelligibility or macOS VoiceOver execution |
| P09, P42, P58 | Scale `backend.json`, `shelves.json`, `desktop/` and `resources/`, including raw native event clocks, workloads, hardware, warmups and all samples | Original numerical ceilings, native acknowledgement bounds, resource/display acceptance and distinction from observer scheduling diagnostics |
| P57, P59–P60 | Candidate package/stage/compiler records, installed inspection and actual payloads, linkage/process records, fresh/upgraded/reopen/reinstall reports | Signature validity, clean-host final signed bytes or full release approval |

The original budgets, workload sizes, failure verdicts and evidence source
identities remain unchanged. A later passing sample does not explain or erase a
previous numerical failure. Equal implementation-file hashes do not make two
different source trees or packages the same candidate.

## Decisions and remaining release holds

The row contract records the existing owner decisions: Windows is deferred;
VoiceOver execution is excluded with no spoken macOS claim;
Japanese/Chinese/Arabic-specific input, IME and shaping qualification is deferred
while valid UTF-8 data and filenames remain unrestricted. In-place legacy
adoption is refused without lifetime exclusion; the supported fallback verifies
an independent library/native-profile/recovery clone and leaves legacy authority
directed at the source. These decisions do not authorize new exclusions.

Every row retains the exact-final-candidate and independent-review hold under
19.6/19.7. P59/P60 additionally expose the human-controlled public Mac signing and
final-byte checks under 18.2/18.3/18.10/18.11. The audit can record observed internal
behavior while those release gates stay open. Complete-parity, promotion and
Go-retirement claims require all mandatory subcases and final package gates to
pass; this command cannot clear them.

Record independent findings and their resolutions beside the evidence. Bind the
final review to the exact source/target/package checksums, including all three
targets and both fresh/upgraded runs. Do not commit a post-build pass claim back
into its own source and start a self-referential rebuild cycle.
