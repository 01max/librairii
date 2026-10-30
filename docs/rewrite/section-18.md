# Task 18: native platform packages

Integration base: `0e3b67da0af8836af4b89e3079c38a9997d0e7d8`.
Branch `codex/rewrite-18-platform-packages` targets
`rewrite-in-rust-native-desktop`.

Task 18 is in progress. A staged payload, unsigned application or successful
build does not qualify an installed distribution. Package, GUI, clean-host,
reopen/recovery, final-byte signature and independent review gates remain
explicit until their actual reports pass. Existing Go release commands remain
available during the rewrite.

## Release entry point

`cargo xtask release plan --target <triple>` reads the existing product `VERSION`,
source commit/tree/epoch, locked graph, shipping features and toolkit pin. It
marks a dirty worktree visibly and performs no build or qualification. The
supported triples are the three recorded in `.config/rust-checks.toml`.

`cargo xtask release build --target <triple> --output target/native-release/<new-name>`
requires a clean committed tree, matching native host and pinned Rust/Cargo. It
refuses ambient Rust compiler/profile overrides (including Cargo aliases),
inherited Cargo configuration layers, existing outputs and linked or
escaping output paths. Compiler output must identify exactly one production
binary with opt-level3, assertions disabled and overflow checks disabled. The
copied executable must have the exact architecture and report the same embedded
version, source/tree, lock digest and target through `--build-info`.
The repository's alias-only `.cargo/config.toml` is the only admitted Cargo
configuration. A builder with global or ancestor Cargo config must use an
isolated checkout and Cargo home without those files.

The `payload` contains the executable, embedded-identity record, original icons,
compiled dependency inventory, SPDX document and required notices. `stage.json`
records every payload file's final byte length, digest and executable mode.
`cargo xtask release verify-stage <directory>` checks those exact bytes. Neither
command opens the product GUI or reads a normal user profile.

`--version` and `--build-info` return before profile, menu or window startup.
Ordinary development builds explicitly report absent release fingerprints;
they cannot satisfy the release builder's identity check. The source product
version remains `0.5.0`; this task does not invent a new release version.

## Inventory and reproducibility boundaries

The SPDX2.3 document uses the actual release compiler-artifact package identities
and features, including build-time inputs. It excludes uncompiled development
packages even when Cargo metadata lists them. Registry checksums come from the
locked graph. Local components identify their repository commit and relative
manifest. The pinned native Skia source/notice union is recorded separately,
without asserting that every union member is linked on every target.

Notices are freshly collected from the actual package sources and checked-in
derivative/supplemental/native notice inputs. Old mutable `target/quality`
outputs are not copied into the payload. The selected Slint royalty-free route
retains its license text and actual top-level AboutSlint attribution. The
product's own license is not reassigned; SPDX records `NOASSERTION` where the
repository makes no declaration.

## Linux archive

On the native Ubuntu24.04/glibc2.39 builder, `cargo xtask release package-linux
--stage <current-stage> --output target/native-release/<new-package>` assembles
a versioned tar.gz. It refuses stale source/graph stages, requires the actual
glibc/distro baseline, checks ELF dependencies/interpreter/version requirements,
validates the desktop entry and seals the final archive bytes. The archive
contains `bin/`, `share/applications`, hicolor icons, notices/SBOM, installation
instructions and a concrete native runtime prerequisite record. The portable
layout supports an explicit application prefix; its `bin` and `share` must be
available to the desktop through PATH and XDG_DATA_DIRS for integration.

The system library allowlist excludes browser runtimes and build-directory
dependencies. Font, graphics and portal libraries loaded at runtime have
separately declared Ubuntu packages. The platform TLS verifier also requires
`ca-certificates`, including when an isolated acceptance catalog uses loopback
HTTP, because the client initializes its platform trust store before requests.
Installed X11/Wayland and rendering tests
must establish those prerequisites in use; inspection alone cannot pass them.

## macOS application and disk image

`cargo xtask release package-macos --stage <current-stage> --output
target/native-release/<new-package>` requires the matching native host (real
Intel hardware for the Intel target). It copies the verified thin executable
and full notices/SBOM into a versioned `Librairii.app`, supplies macOS15.0 metadata,
native icons and embedded-catalog declarations, and checks all linked libraries
against system paths. It builds and verifies a compressed read-only DMG with
Apple's `hdiutil`. The final artifact inventory follows application assembly
and the internal ad-hoc signature.

The `-unsigned.dmg` suffix and signing record deliberately distinguish these
internal candidates from Developer ID distribution. An ad-hoc integrity check
does not establish notarization, stapling or Gatekeeper acceptance. Both macOS
public-release gates remain incomplete until the final signed bytes pass those
checks and installed clean-host GUI acceptance.

`cargo xtask release inspect-package --directory <artifact-directory>
--expected-source <externally-selected-commit> --target <triple> --output
target/native-release/<new-installation>` verifies the distributed checksum,
extracts the Linux archive or copies the mounted read-only DMG application into
a fresh installation, compares every installed payload byte/mode, then checks
native linkage and the actual installed executable's embedded identity. It
runs from the installation directory and requires no Cargo invocation by the
installed product. This command performs headless inspection only; its receipt
cannot pass GUI qualification or public signing.

The `Rust native packages` workflow builds on `macos-15`, `macos-15-intel` and
`ubuntu-24.04`, then downloads each exact candidate to a separate installation
job. Installed-host validation needs no Rust toolchain and refuses an Intel
result running through Rosetta. The installation job also runs the checksum-keyed
production GUI workflow; byte inspection and GUI qualification remain separate
receipts and gates.
The pinned official download-artifact6.0.0 commit
`018cc2cf5baa6db3ef3c5f8a56943fffe632ef53` was checked against its official tag
and GitHub's valid release-commit signature before use; existing checkout,
cache and upload revisions remain the previously reviewed pins.

Portable archive assembly sorts paths, rejects links/special files, normalizes
UID/GID/names/modes and uses the source commit epoch with a fixed gzip header.
Repeated assembly of identical payload bytes is deterministic. This is distinct
from claiming bit-identical notarized DMGs: signatures, trusted timestamps,
notarization tickets and OS image metadata have their own final-byte evidence.
The artifact record and checksum are generated after packaging, and packaging
alone never sets public-release or runtime-qualification status to passed.

The SPDX field contract follows the official
[document](https://spdx.github.io/spdx-spec/v2.3.1/document-creation-information/),
[package](https://spdx.github.io/spdx-spec/v2.3.1/package-information/) and
[relationship](https://spdx.github.io/spdx-spec/v2.3.1/relationships-between-SPDX-elements/)
specifications.

## Credentials and remaining gates

The initial read-only capability check found zero repository secret names and
zero valid local codesigning identities. No secret value was requested or read.
Public Mac signing/notarization/stapling remains incomplete. Unsigned build and
qualification work proceeds independently. Signing execution must isolate
credentials from untrusted pull-request builds and verify the final distributed
bytes, following Apple's
[notarization workflow](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

The intended baselines remain macOS15+ on ARM and real Intel hardware, and
Ubuntu24.04 x86-64 with declared glibc/system dependencies. These are qualification
targets until actual installed-package reports establish support. Windows is
outside this change. Task19 release-readiness and Task20 runtime retirement do
not become complete through packaging alone.

## Installed GUI isolation

`LIBRAIRII_PACKAGED_ACCEPTANCE=1` enables the installed product's fixture
admission. It requires all four explicit variables: `LIBRAIRII_ACCEPTANCE_ROOT`,
`LIBRAIRII_ACCEPTANCE_PROFILE`, `LIBRAIRII_ACCEPTANCE_LIBRARY` and
`LIBRAIRII_ACCEPTANCE_CATALOG`. The owner must be a physically unaliased fresh
temporary fixture directory with its exact marker; profile/library must be its
explicit separate children. The catalog route must be HTTP on a numeric
loopback address. Missing or invalid inputs fail before native menus, a worker,
normal home discovery or a window. Profile selection uses explicit locations;
qualification does not repurpose the process HOME variable.

Saved native/legacy profile roots must remain in that fixture. New picker paths
are checked lexically at bounded UI intake and physically again on the worker
before command execution, including link/hardlink substitution. Ordinary
production startup and its fixed catalog endpoints retain their existing
behavior. This route changes only isolation and networking; the installed
production binary still constructs its normal GUI, worker and workflows.

The `cargo xtask release qualify-gui --inspection <inspection.json> --output
target/native-release/<new-run>` facade runs only on disposable CI. The
installed application is copied byte for byte into its temporary fixture and
launched from an unrelated empty working directory. Public AX on macOS and
AT-SPI on Linux operate the actual product window, native pickers and menus.
Screenshots, control observations, SQLite results, managed/exported hashes,
native file-manager location/retirement and process shutdown evidence are
retained against the artifact checksum. Missing checkpoints fail qualification.

Each target runs fresh English/GPU-API and upgraded French/forced-CPU cases;
Linux repeats both under private X11 and Wayland desktops. The Linux observer
image contains runtime libraries, the GTK portal and Thunar, with no build or
archive tools. It runs read-only with no external network and no source checkout
mounted. macOS restricts application writes/networking and executable children
with a per-process sandbox; qualification observers remain outside it. Five
owned launches cover the complete workflow, ordinary reopen, a real in-flight
catalog SIGKILL/reopen, missing-location recovery and application-only
uninstall/reinstall with retained library/profile bytes.

The driver implementation and its custody checks are prepared. These commands
do not count as passing installed evidence until their actual native CI reports
pass. All Task18 GUI and public-signing checkboxes remain open at this point.

## Verification

The three required Rust integration tests execute the actual product's identity
commands with no display or normal profile environment and deliberately invalid
startup configuration, including missing packaged-acceptance roots. Three
additional Rust checks exercise fixture/catalog admission, persisted foreign
roots and picker substitution before controller execution. Eighteen additional
Python checks, plus five packaged-GUI custody checks, cover compiler aliases
and inherited configuration, no-clobber paths,
link refusal, exact architecture/profile/embedded identity, stable archive bytes,
final-byte records and actual compiled-graph selection. They are registered in
the cumulative Python runner's required baseline. Existing identities and
original source-index/baseline pins remain preserved through explicitly recorded
additions; no source, algorithm or historical parity row is reclassified.
The fixture gate also physically checks the derived legacy fallback root and
refuses any persisted native relocation journal before startup. These cases
exercise regular relocation, missing-location recovery and interrupted metadata
work; accepting a persisted relocation journal would require a separately
bounded fixture authority check.

Each export freezes its independently selected UUIDs, original filenames,
managed hashes/sizes, group provenance and eligible/skipped item outcomes before
starting. Completion requires the exact new durable operation and exact output
set, including the expected Broken-story skip in the upgraded fixture. Partial
copies or a different scope cannot qualify. Reopen also runs with the catalog
listener stopped and connection refusal observed, at an actual native display
scale of 1.25 after the initial scale 1 launch. Native scale/geometry diagnostics,
visible controls and screenshots must agree. SIGKILL recovery requires the
actual signal exit status and positive output-reader retirement.

## Protected Mac signing

The manually triggered `Sign reviewed native Mac candidate` workflow accepts an
exact reviewed source commit, unsigned package run, native target and independently
checked DMG SHA-256. It runs only from `main` or the Rust integration branch.
Signing code is checked out from the trusted workflow revision separately from
the candidate source. Pull-request jobs receive no signing credentials.

Before enabling signing, the owner must configure the `native-macos-signing`
environment with a required reviewer and deployment restricted to those trusted
branches. Supply environment secrets `NATIVE_SIGNING_KEYCHAIN_BASE64` (an encrypted,
dedicated keychain containing only the selected Developer ID Application identity
and a notarytool credential profile named `librairii-notary`) and
`NATIVE_SIGNING_KEYCHAIN_PASSWORD`. Use GitHub's secure secret entry, never PR
comments or chat. Set nonsecret repository variables `NATIVE_MACOS_TEAM_ID` and
`NATIVE_MACOS_CERT_SHA1` to the independently selected team and certificate.
Prepare the dedicated keychain with Apple's Keychain Access/security tools and
`notarytool store-credentials --keychain`; no normal login keychain is exported.
No credentials have been provisioned or used by this task.

The entry point consumes both secret environment variables before any preflight
child process. The credential-bearing job only copies/verifies candidate bytes
and invokes trusted Apple tools; it never executes the candidate, including an
identity command before keychain creation. An encrypted keychain copy exists
only in a private fresh runner-temporary directory, is selected explicitly by
signing/notary tools and must retire positively before success.

The app receives a timestamped Developer ID signature with hardened runtime.
An accepted app ZIP submission leads to stapling/validation of the actual app;
the resulting DMG is separately signed, accepted, stapled and validated. Final
checksums/payload inventories are produced afterward. A separate credentialless
matching native job installs those final bytes and independently checks the
selected certificate/team, both tickets, Gatekeeper, linkage and full GUI cases.
Signing alone never sets public-release qualification true. Both architectures
must complete this flow independently; Intel still requires actual Intel hardware.

This sequence follows Apple's [custom notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)
and [distribution packaging guidance](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution).
The new manual workflow becomes dispatchable once GitHub has registered its
trusted workflow definition; it has not been executed or claimed qualified.

## Current retained diagnostics

[Run 41053772648](https://github.com/01max/librairii/actions/runs/41053772648)
built all three native packages from provider merge source
`9acfef8753fbed84ec192dae78653e58a97e8e3c`. Separate installed-byte inspections
passed on all three hosts. Provider archive and inner package hashes were
independently verified. The package digests are:

| Target | Package SHA-256 |
| --- | --- |
| macOS ARM | `c6a582c65a58fc3a1419002c7aabb3180fa7152b666e45cfe05dcae152bec4b1` |
| macOS Intel | `9c113abf9304c60d4bab89c7a5d98201597fca4d6551784ba00fb2efea475d54` |
| Linux x64 | `679fcc18103b3adc6bcc75ce1734e21aec92bc076300611d0115aafdc7974b96` |

Actual GUI acceptance has not passed. Retained failures led to separate fixes
for AppKit picker edit routing, the Linux trust-store prerequisite, private
AT-SPI activation, native picker selection, exact text clearing, Shelf label
counts and visible-control navigation. All earlier failed artifacts retain
their original verdicts. Observer source and file hashes are recorded separately
from candidate source, so a new observer cannot silently replace the package.

The cumulative required baseline contains 105 Python identities, all passing;
original identities and frozen fixture/oracle inputs remain intact. Independent
Standards and Spec source reviews are recorded separately from native evidence.
[Rust quality run 41053772592](https://github.com/01max/librairii/actions/runs/41053772592)
and [scale run 41053772763](https://github.com/01max/librairii/actions/runs/41053772763)
passed on all three native targets. These checks do not replace installed GUI
or real signing evidence.

The subsequent [scale run 41060966731](https://github.com/01max/librairii/actions/runs/41060966731)
and [foundation run 41060966982](https://github.com/01max/librairii/actions/runs/41060966982)
passed on all three hosts. Fresh Intel foundation input p95 was40.688995ms,
with none of64 observations above100ms. The historical failed measurements
below retain their verdicts; a sole cause has not been established.

[Quality run 41060966741](https://github.com/01max/librairii/actions/runs/41060966741)
passed on Intel and Linux but failed on ARM when an optional Cancel raced an
already completed first import. The observer now records that exact terminal
receipt separately. Cancellation of the second live import and its positive
cancelled-item assertion remain mandatory. Fresh qualification is required.

The [ARM installed replay 41067069023](https://github.com/01max/librairii/actions/runs/41067069023)
passed all four export scopes, exact Finder location and window retirement,
then failed relocation because observer SQLite readers retained sidecar files.
The observer now closes its read-only connections explicitly; a real WAL
regression failed before this correction for successful and failing queries
and passes afterward. Storage closure requirements are unchanged. The report
remains failed until a fresh complete installed workflow passes.

[Native foundation run 41053773048](https://github.com/01max/librairii/actions/runs/41053773048)
passed on Linux and ARM but failed the existing Intel 100ms input budget:
p95 110.303635ms, with five of 64 observations above 100ms. The original raw
artifact remains failed and retained. The earlier Linux foundation run measured
p95 158.28226399992218ms and also remains failed; its later fresh passing run
does not rescore it. No workload, sample count or budget was relaxed.
Public signing, notarization, stapling and final signed-byte launch remain
unexecuted because credentials have not been provisioned. Task18 remains open.
