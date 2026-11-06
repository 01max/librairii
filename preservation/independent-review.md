# Independent section 1 audit

The complete section 1 implementation at `cb699ab42225c322611c85889f9dbde638dba150`, against integration base `41267f6c09cb392371fdb17310914d9507a9d318`, passed independent inventory, effective-spec and corpus review. This closes the substantive 1.9 audit. Its completion and ledger commits still require a new exact-head external review and provider checks before admission; the earlier approval does not approve later commits.

## Review and corrections

[AO native review](https://github.com/01max/librairii/pull/39#pullrequestreview-5587048588), run `80e2033d-eb86-49b6-a4e9-0fb068082290`, approved that implementation on 2026-10-20 with zero required standards or spec findings. It reviewed the complete PR scope, retained the earlier review of unchanged files, independently checked producer/fixture hashes and representative SQLite integrity/foreign keys, and accepted the effective-requirement P23 mappings. The reviewer did not execute project programs, tests or builds.

The [initial required finding](https://github.com/01max/librairii/pull/39#discussion_r4326423344) exposed marker-only relocation journals that could not preserve the supplied library. Dedicated correction `30efe66aea3afe76197bb44d1cfb43274058aed8` captures the finalized complete library using the actual Go `preferences.Manifest`. It retains three realizable positive recovery cases and two explicitly negative incomplete-manifest cases, with copy/verification, profile authority, durable-byte preservation, cleanup and repeat-reconciliation outcomes. The thread is resolved. Dedicated follow-up `cb699ab42225c322611c85889f9dbde638dba150` maps Broken mutation, guarded Undo and lossless legacy migration to P23. Both corrections were accepted by the independent rerun.

## Independent execution

Integration owner `librairii-2` independently executed the same implementation head in a clean detached checkout with its own object database and isolated synthetic data/config/cache roots. The report remains a nonshipping AO artifact at `/Users/maxime/.ao/data/artifacts/librairii-2/section-01-integration-verification.md`, SHA256 `e9f9dcd3c19554e5ec475c5a861f52a6e23cca78bc3df973d08a66fd385903f0`.

| Command family | Independent outcome |
| --- | --- |
| `python3 preservation/tools/verify-source.py <frozen-oracle>` before and after | All 436 hashes passed at `73a93303a9a44cf0a92eaf525331561ed8c04460`. |
| `python3 preservation/check.py preservation/corpus` | All 146 hashed files and semantic witnesses passed. |
| `python3 preservation/generate.py --oracle <frozen-oracle> --output <new-temp-root>` twice | Both exports executed actual frozen Go APIs and succeeded. |
| `python3 preservation/check.py <export-a> --compare <export-b>` and committed corpus versus export | Every byte, including the manifest, matched. |
| Real Go relocation replay within both exports | Three positive cases preserved complete authoritative bytes, cleaned owned artifacts and succeeded on repeat; incomplete copy returned `library_missing`; incomplete cleanup retained both roots/journal and returned `cleanup_pending` twice. |

Execution ran with Go 1.26.5 darwin/arm64 and Python 3.14.7, `GOPROXY=off`, `GOSUMDB=off`, `GOTOOLCHAIN=local`. The frozen oracle and independent checkout remained clean. No production library/profile or production endpoint was used. Corpus manifest SHA256 was `a59f2194cf5c1ad317485a8714e703c12fe161bfad0190a67ba5816ea7f46e53`.

## Cumulative evidence and remaining gates

The implementation owner's independent clean-head `make check` passed at the reviewed head, including 207 frontend tests and responsive, visual, accessibility and performance checks. The cumulative Go JSON run retained 596 test/subtest passes and one explicit cross-volume skip; existing retained relocation/restart/corruption tests also pass. The earlier oracle cleanup timeout remains documented as `GO-BASELINE-001`, with focused and full unchanged reruns passing. Current reviewed-head provider [verify](https://github.com/01max/librairii/actions/runs/39444871130/job/118703997956) and [Linux release evidence](https://github.com/01max/librairii/actions/runs/39444871128/job/118703991704) both passed. These results identify their tested implementation head and do not stand in for post-completion CI.

Strict rewrite validation and all 12 effective-domain validations pass. Each of the 77 effective requirements maps to preserved parity IDs; all 60 P rows have baseline owners and proposed Rust evidence. The approved UI digests, source/test/doc inventory, scoped reconciliation and independent review are linked in `evidence-index.json`. The 12 canonical effective-spec hashes match their shipping snapshots. The separate spec repository receives only the authorized local 1.9 checklist update; no commit, publication or archive occurs there.

Missing resources remain explicit: a second local volume was not supplied through `LIBRAIRII_TEST_DESTINATION_ROOT`; native Rust startup/CPU/RSS/package-size, real Intel macOS, Linux/Windows native rendering, screen-reader, signing and adoption/rollback qualification remain future gates. All Rust P-row platform states remain unverified. Section 1 does not implement or complete the remaining rewrite sections.

Final admission uses the external AO/GitHub verdict and provider checks matching the actual published PR head after these evidence commits. No commit attempts to embed its own SHA or approve itself. AO approval is internal review evidence, not an independent-account GitHub approval or merge authorization. Integration owner `librairii-2` checks final admission; this worker never merges PR39 or global PR37.
