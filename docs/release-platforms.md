# Release platform matrix

Workflow reviewed: 2026-10-20. Platform evidence below is recorded separately
from this documentation review; it is not a new qualification run.

Librairii supports macOS and Linux desktop distributions.

Release qualification runs on the target operating system and invokes its
host-native platform services. A cross-compiled binary, frontend-only test, or
successful artifact build does not qualify a target. Every qualified row must
independently build its distribution artifact, launch the actual packaged
executable, commit the embedded React render, create and reopen SQLite state,
pass host-native dialog and reveal acceptance, run the complete headless
story-library smoke, shut down cleanly, and produce a verified SHA-256
checksum. The complete headless story-library smoke is identical on both
targets.

The packaged acceptance driver opens the production platform dialogs, selects
only prevalidated isolated fixture paths through a host-native automation hook,
validates the paths returned by Wails, and records separate native import,
destination, and reveal evidence before domain work may continue.

## Targets and artifacts

| Distribution target | Verification host | Artifact | Command | Status |
| --- | --- | --- | --- | --- |
| macOS 15, arm64 | macOS 15.7.7 arm64 | versioned DMG | `make verify-packaged-acceptance` | Previously passed locally; rerun for each release |
| Linux x64 with WebKitGTK 4.1 | GitHub-hosted Ubuntu 24.04 x64 under Xvfb | versioned portable `tar.gz` | `make verify-platform-linux` | Host-native CI gate |

The macOS artifact is a signed application bundle in a DMG with an
`Applications` link. Its gate installs into an isolated location, launches the
installed application, exercises native panels and Finder reveal behavior,
reopens the same SQLite state, and proves that removing the installed
application retains user data.

The Linux archive contains the Wails executable and expects the distribution
to provide GTK 3 and WebKitGTK 4.1. It is intentionally not a generic package
for every Linux ABI.

## Build Linux artifacts from macOS or Linux

Run `make linux` with a running Docker engine and the Buildx plugin. Docker
Desktop includes both. The command builds the current local checkout for
`linux/amd64` inside Ubuntu 24.04, using Go 1.26.5, the Node version in `.nvmrc`,
and the Wails version in `.wails-version`. Host toolchains and `make setup` are
not needed. Apple silicon hosts use Docker's amd64 emulation.

The Docker build exports these files into `build/bin/`:

- `Librairii`;
- `Librairii-<VERSION>-linux-amd64.tar.gz`; and
- `Librairii-<VERSION>-linux-amd64.tar.gz.sha256`.

Build inputs exclude host dependencies, existing output, Git history, and local
environment files. Compilation and generated bindings stay inside the container.
The build verifies the ELF x64 architecture, resolves linked libraries in the
Ubuntu build environment, and verifies the archive checksum. It does not run the
native UI acceptance scenarios below; use `make verify-platform-linux` on its
Linux verification host to qualify the release.

The Linux gate runs the packaged scenario in one isolated D-Bus/Xvfb desktop
session. It registers a verification-only `inode/directory` handler that
delegates to PCManFM, then requires the live PCManFM process and its command
line to contain the exact validated export directory before the gate passes.

## Evidence contract

Each platform task is one command with no dependency on another platform's
artifacts:

1. Assert the host OS/architecture and exact pinned Wails CLI.
2. Build fresh production frontend assets and a clean, trimmed Wails binary.
3. Validate the native executable architecture and distribution artifact.
4. Install installer-based distributions into an isolated user-owned location
   and validate their installed payload.
5. Launch the installed or packaged executable and require a committed React
   render, a clean lifecycle, and no external sidecar.
6. Reopen the same SQLite database and require the smoke story and saved
   Collections (`foundation-smoke` retains the legacy `-expect-shelves` option
   name for this count).
7. Run native multi-file, directory, and reveal-adapter acceptance.
8. Run the complete headless composition and root release contracts.
9. Remove temporary installed application files without deleting user-owned
   SQLite data.
10. Verify the distribution artifact's SHA-256 checksum.

The qualification implementation is:

- macOS: `scripts/verify-packaged-acceptance`;
- Linux: `scripts/verify-platform-linux`; and
- CI orchestration: `.github/workflows/ci.yml`.

Failed or cancelled jobs publish no artifact.

## Settings and library relocation

Both packaged verification scripts also run `scripts/verify-settings-acceptance`
against their platform's application. This covers first-run confirmation,
English/French Settings menus and accelerators, native folder selection,
library relocation, and export after a move. macOS evidence and the Linux
host limitation are recorded in
[Settings and library location verification](verification/settings-and-library-location.md).
