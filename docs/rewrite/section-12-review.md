# Task 12 independent review

Fixed base: `22408a9e32efed073acc58cce1b25eedf1593236`, the merged Task 5 epic.
Reviewed implementation: `b2c97775f6a73f8de6148dda7d08f322f3d89461`.
Independent read-only Standards and Spec reviewers assessed the complete Task 12
diff, the native-path delta and the exact committed correction. Earlier partial
reviews and fixes remain in the rebased history. No finding remains open.

## Standards

Final result: approved, zero open actionable findings. The review found one P2
lossless-path boundary violation: removal still decoded managed and journal paths
into UTF-8 String values although Task 5 imports and Task 12 exports retained raw
native names. Commit b2c9777 resolves it with the existing OperationPath/raw TEXT
helpers and native filename components through move, restoration and recovery.

The reviewer verified that raw export names/paths stay intact through creation,
immutable witnesses, digesting, publication and History. Display conversion is
separate and preparation accounting includes the raw buffers. Prior custody,
sticky fencing, refined Shelf provenance, cleanup and accounting fixes remain
intact. No additional consequential heuristic smell was found.

## Spec

Final result: approved, zero open actionable findings across 12.1–12.9.
The same P2 removal gap violated the requirement to retain lossless OS paths
through core operations. Its correction and expanded native crash/recovery tests
resolve the finding. Review also covers all explicit export scopes, ordered
UUID unions, unavailable/Broken skips, stale and custody-wait revalidation,
no-clobber publication, cancellation, retained uncertainty, removal/tag/Shelf
semantics and diagnostic privacy/size boundaries.

The real native import→inspect→export→reopen test closes the former Task 5 gap.
It exercises seven format labels through actual native import and export workers,
SQL and native close/reopen, compares output metadata/bytes and raw stored names,
then verifies collision refusal and removal into recoverable trash. Linux uses
malformed UTF-8 filename bytes; macOS uses Unicode/spaces/case. Legacy/source
archives and database bytes remain unchanged. The existing process-death matrix
now repeats intent, trash move, deletion and corrupt-trash boundaries with both
ordinary and difficult native managed names.

The reviewers rechecked the exact committed source at b2c9777. Their approval is
an implementation/review judgment; required provider execution evidence remains
separate. No GUI, package, Windows, device-transfer or global cutover claim is made.

## Verification

Final macOS ARM checks passed on the reviewed b2c9777 implementation:

- `cargo xtask check`: 534 tests passed, zero skipped; 19 doctests passed;
  formatting, all-target Clippy, both locked dependency/advisory inventories,
  source/notice checks, cumulative discovery and preservation passed.
- `cargo xtask fuzz`: identity 9,650,906 inputs and import_boundaries 459,609
  inputs, 31 seconds each, peak RSS 301 MiB and 347 MiB. Both passed the pinned
  time/input/RSS limits without crashes, timeouts or resource failures.
- Focused removal tests passed, including eight actual child-process kills across
  four boundaries and two native-name variants. The complete real roundtrip also
  passed across seven formats and subsequent removal into retained trash.

Required native Linux x86-64, macOS ARM/Intel and focused provider checks must
also pass on the pushed revision before this PR is marked ready. The Linux raw
non-UTF-8 cases remain provider execution evidence until that run succeeds.

An earlier final-check attempt exhausted local disk space while compiling/linking;
it supplied no test failure or successful fuzz evidence for the interrupted run.
Only generated Rust incremental caches under this checkout's target directories
were removed. Sources, retained fixtures and evidence reports were preserved, and
the same checks were rerun with incremental compilation disabled.
