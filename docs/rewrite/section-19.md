# Task 19: native documentation and release readiness

Integration base: `4b7cd37cc6cbafdcdd4d86f35661018c9c7dd027`, after
[PR69](https://github.com/01max/librairii/pull/69). This section separates the
completed product documentation from the remaining final release gates.
It does not authorize promotion or retirement of the Go application.

## Product documentation

- [Development](../development.md), [architecture](../architecture/0005-rust-native-desktop.md)
  and [platform support](../release-platforms.md) describe the Rust candidate,
  pinned toolchain, source checks and exact native runtime prerequisites.
- The [user guide](../user-guide.md) follows native navigation, dialogs,
  export scopes and recovery controls. Privacy, byte-preserving imports/exports,
  explicit Shelf decisions, Broken skips and manual application-trash recovery
  remain part of the contract.
- [Transition instructions](../native-transition.md) give exact backup paths,
  one-time library-bound preference transfer, independent adoption, safe rollback,
  post-backup edit reconciliation and foreign filename/profile/journal limits.
- [Native screenshots](../assets/README.md) retain original CI PNG bytes and
  record provider ZIP, package, build source and image hashes. Historical Go
  screenshots and all frozen behavior/source fixtures remain available.

## Parity audit index — 19.1

[`section-19-parity.json`](section-19-parity.json) indexes **all 60 IDs**, each
with the retained contract, implementation, executable check source, delivery
evidence, observation topics, named accountable owner and separate A/I/L status.
`01max` is the accountable repository/release owner; the original workstream
assignments remain visible. The audit checks that each referenced source exists;
the delivery reports explain its assertions, fixtures and prior independent
reviews. A source filename or passing CI summary alone is not proof that every
subcase has passed.

The index names the pulled integration base and its tree. Candidate run references
name PR69's **final** head `21f32ed968cd4e66bfde6817e0f77b4c93dac810`, whose actual
build source is `bd7327b8b600f34b21c8a44bac803551b4e8e05b`, tree
`7bab4be1c1869a286364b42260df648ae4305dbe`. That tree equals the integration base.
The later Task 19 documentation revision must obtain its own required CI results;
neither the old source91 results nor this index qualifies new package bytes.

| Evidence family | Candidate run | Required scope |
| --- | --- | --- |
| Quality | [41185064457](https://github.com/01max/librairii/actions/runs/41185064457) | Matching ARM, real Intel and Linux hosts; cumulative locked checks, frozen corpus, storage/upgrade/relocation rehearsals, native controls and Linux Orca |
| Scale | [41185064367](https://github.com/01max/librairii/actions/runs/41185064367) | Three-target backend, actual native input/loading/cache and release resources/display evidence |
| Packages | [41185064509](https://github.com/01max/librairii/actions/runs/41185064509) | Three distinct source-bound packages; installed clean/upgraded workflows, reopen, recovery and reinstall |

These runs reported passing required checks for the merged candidate. Raw evidence
retains its actual source, target, binary/package hash, workload, commands,
assertions and failures. Task 18's independently admitted historical reports keep
their original revisions. The Task 19 screenshot extraction additionally admits
the latest-base ARM raw fresh/upgraded package reports; it does not extend that
raw admission to another target or another source.

Final row closure remains pending exact final-candidate evidence and independent
release review. P56–P60 additionally span platform/runtime/performance/package
composition; final public Mac signing is still missing. Keep 19.1 and the final
promotion gates open until every mandatory subcase is reconciled. Do not replace
the planning register's unverified status with a blanket Rust pass.

Recorded owner decisions continue to apply: VoiceOver execution is excluded and
macOS spoken accessibility is not claimed; Japanese/Chinese/Arabic-specific
input, IME and shaping qualification is deferred while valid UTF-8 data/names
remain lossless and unrestricted. Windows is a separate follow-up. In-place Go
adoption is refused because lifetime exclusion of unmodified legacy binaries is
not established; independent library/profile/recovery adoption is required.

## Required final review — 19.2, 19.6 and 19.7

The final release checklist is deliberately incomplete:

- [ ] Finish protected Developer ID signing, notarization and stapling for both
  Mac architectures, then install/launch/inspect those **final signed bytes** on
  matching hosts. Follow [Task 18's signing procedure](section-18.md).
- [ ] Record all required final-source quality, foundations, scale/resilience,
  focused and retained Go compatibility checks, including their aggregates.
  Failed, cancelled, missing or unexpectedly skipped targets do not qualify.
- [ ] Review the exact compiled dependency graph, advisory/license exceptions,
  original notices and About Slint attribution. Bind target-specific SPDX,
  compiler-artifact graph, features and provenance to the final package.
- [ ] Assemble the commit/target/final-SHA evidence index described below,
  retaining raw packaged, recovery, performance and accessibility records.
- [ ] Independently review clean and upgraded full rehearsals on all three
  targets, resolve every mandatory parity row and record known limitations.
  Only then can a separately authorized cutover proceed.

Public signing credentials, trusted workflow setup and owner approvals remain
external gates. Unsigned candidate evidence can be completed independently, but
does not fill those signing/final-byte boxes.
