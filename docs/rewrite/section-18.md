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
separately declared Ubuntu packages. Installed X11/Wayland and rendering tests
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
result running through Rosetta. This initial job qualifies byte installation
and headless inspection only. The checksum-keyed GUI reports remain separate.
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

## Verification

The two required Rust integration tests execute the actual product's identity
commands with no display or normal profile environment and deliberately invalid
startup configuration. Seventeen additional Python checks cover compiler aliases
and inherited configuration, no-clobber paths,
link refusal, exact architecture/profile/embedded identity, stable archive bytes,
final-byte records and actual compiled-graph selection. They are registered in
the cumulative Python runner's required baseline. Existing identities and
original source-index/baseline pins remain preserved through explicitly recorded
additions; no source, algorithm or historical parity row is reclassified.
