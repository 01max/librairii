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

### Dependency and attribution review — 19.2

The reviewed graph inputs are `Cargo.lock`, `fuzz/Cargo.lock`, `deny.toml`,
`.config/dependency-exceptions.toml`, `.config/native-toolkit.json` and the
source/notice inventories. Task 19 does not change those inputs. The selected
toolkit remains Slint 1.18.1, Winit 0.30.13 and Skia 0.153.3, including the
reviewed local derivative provenance. The exact build selects the configured
default product features; a hypothetical all-features graph is not its SBOM.

The exception inventory contains **30 exact-version duplicate exceptions,
11 exact-version license exceptions and 5 advisory exceptions**. Every record
has owner `01max`, a reason, tracking link and future expiry at this review;
the first expiry is 2027-01-21. Records and `deny.toml` suppressions match one
for one; broad subtree skips remain empty. These are retained, expiring
exceptions, not new waivers introduced by Task 19.

| Registered advisory exception | Recorded package | Scope of retained rationale |
| --- | --- | --- |
| `RUSTSEC-2026-0192` | `ttf-parser 0.25.1` | Informational unmaintained notice in the qualified toolkit graph |
| `RUSTSEC-2026-0371` | `zbus_names 4.3.5` | Informational unmaintained notice in the qualified toolkit graph |
| `RUSTSEC-2026-0370` | `zvariant 5.15.1` | Informational unmaintained notice in the qualified toolkit graph |
| `RUSTSEC-2026-0372` | `zvariant_derive 5.15.1` | Informational unmaintained notice in the qualified toolkit graph |
| `RUSTSEC-2025-0141` | `bincode 2.0.1` | Optional support remains in the lockfile; absent from all three candidate compiler graphs |

This table describes the committed exception rationale. Fresh `cargo-deny` and
`cargo-audit` results against the current advisory database remain required for
the final source, including the separately locked fuzz graph. `cargo xtask check`
validates expiry, exact suppression equality, both policy graphs and inventory;
new advisories do not inherit an exception from this table.

The Slint royalty-free route retains original terms plus the actual **About
Slint…** component and website action. Original Rust licenses, supplemental
derivative notices and the pinned native Skia notice union ship with packages.
Product licensing remains undecided (`NOASSERTION`); this review does not invent
a product license. SPDX distinguishes actual Cargo compiler inputs, including
build-time dependencies, from the conservative native-source notice union;
that union is not a claim that every component is linked on every target.

An offline audit of the three source-bound candidate archives verified their
provider and inner package hashes, clean source/tree, lock/toolkit fingerprints,
empty product feature selection and actual compiler records. Each Mac candidate
has 400 distinct compiled package IDs and 893 inventory files; Linux has 478
compiled IDs and 1,009 inventory files. Mac disk-image contents still require
their matching-host installation inspection; this offline metadata audit does
not mount or launch them.

For Linux, the audit additionally reads the actual tar contents: all 1,009 files
match recorded bytes/modes/hashes, 478 dependency rows match the compiler graph,
the SPDX document contains 490 packages (product plus Cargo inputs and native
source union), and all 13 pinned native notice files plus Slint terms match their
original bytes. Final signature, linkage and running-process claims still come
from the separately keyed platform evidence.

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

### Candidate manifest and final-byte handoff — 19.6

[`section-19-candidates.json`](section-19-candidates.json) indexes the actual
merged-base source, ordered source parents, target, inner package byte count and
SHA-256, provider ZIP checksum, compiler graph, signing state and supporting
quality/scale/foundation/package artifact metadata. It states exactly which raw
scopes were audited at this step. It is an unsigned/internal candidate index;
all three `public_release_qualified` values remain false.

| Target | Merged-base inner package SHA-256 | Public signing state |
| --- | --- | --- |
| `aarch64-apple-darwin` | `13994141820cb2fec600837a7a734c2e9280d18239c959407cbcca4b0d850287` | Ad-hoc; Developer ID/notarization/stapling pending |
| `x86_64-apple-darwin` | `978163d3d684b94afd37d5dc1d6111ba0454acc7582332972efb602a15fc8b9f` | Ad-hoc; Developer ID/notarization/stapling pending |
| `x86_64-unknown-linux-gnu` | `4ffcfcadffb4747bbebf2ef0d46516ff9faaee3efaa38e203f1df5e5db641595` | Linux portable archive; no Mac-signing requirement |

These are inner distribution hashes, **not provider ZIP digests**. Signing can
change the distribution bytes, so final signed DMGs need new hashes and fresh
credentialless matching-host installed evidence. Foundation artifacts keep their
own source `21f32ed`; package/quality/scale artifacts name merge source `bd7327b`.
Even equal trees do not authorize relabeling embedded source identities.

For a future final candidate, preserve the existing `artifact.json`,
`inspection.json`, packaged qualification reports, compiler records, SPDX and
original notice inventories. Index the exact final source/target/distribution
hash alongside raw recovery/handover, performance/resources and accessibility
reports, assertions, limitations and both independent reviews. Provider metadata
alone cannot replace raw seals, numerical gates or actual native observations.
Failed and cancelled runs remain failures and are retained beside later passes.
Archive evidence before finite provider retention expires.

The final index belongs with the CI/release evidence for those bytes. Do not
change a source commit merely to claim that evidence generated after its build
was already inside it. A documentation-only PR still needs its own required CI;
its result does not transfer to a later release candidate automatically.
