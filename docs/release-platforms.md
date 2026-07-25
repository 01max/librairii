# Release platform matrix

Status date: 2026-07-25

Release evidence is host-native. A cross-compiled binary, a frontend-only test,
or a successful installer build does not by itself qualify a target. Every row
must independently build its distribution artifact, launch the actual packaged
executable twice, commit the embedded React render, create and reopen SQLite
state, pass host-native dialog and reveal acceptance, run the complete headless
story-library smoke, and produce a verified SHA-256 checksum.
The packaged acceptance driver opens the production platform dialogs, selects
only the prevalidated isolated fixture paths through a host-native automation
hook, validates the paths returned by Wails, and records separate native
import, destination, and reveal evidence before domain work may continue.

## Priorities and artifacts

| Priority | Distribution target | Verification host | Artifact | Command | Status |
| --- | --- | --- | --- | --- | --- |
| 0 | macOS 15, arm64 | macOS 15.7.7 arm64 | versioned DMG | `make verify-packaged-acceptance` | Passed on the current host |
| 1 | Windows x64 | GitHub-hosted Windows Server 2025 x64 with WebView2 | per-user NSIS installer | `make verify-platform-windows` | Defined as a host-native CI gate |
| 2 | Linux x64 with WebKitGTK 4.1 | GitHub-hosted Ubuntu 24.04 x64 under Xvfb | versioned portable `tar.gz` | `make verify-platform-linux` | Defined as a host-native CI gate |

Windows is first after macOS because it adds the most distinct packaging and
runtime boundary: PE/NSIS, WebView2, Explorer, and Windows native dialogs.
Ubuntu Linux follows to cover ELF/dynamic dependency validation, WebKitGTK,
XDG storage, `xdg-open`, and headless desktop launch. macOS Intel/universal,
Windows arm64, and Linux arm64 are not release targets for `0.1.x`; adding one
requires a new matrix row and the same evidence.

The Windows installer is built with user install scope and expects a supported
WebView2 runtime. The gate installs it silently into an isolated user-owned
directory, validates its current-user uninstall registration and both
shortcuts, and runs acceptance from the installed copy. It then uninstalls and
requires the install files, shortcuts, and registration to be gone while the
SQLite library remains. Its packaged executable is verified as an amd64 PE
image using the Windows GUI subsystem, so it does not open an extra console
window. The Linux archive contains the Wails executable and expects the
distribution to provide GTK 3 and WebKitGTK 4.1. It is intentionally not a
generic package for every Linux ABI.

The Linux gate runs the packaged scenario in one isolated D-Bus/Xvfb desktop
session. It registers a verification-only `inode/directory` handler that
delegates to PCManFM, then requires the live PCManFM process and its command
line to contain the exact validated export directory before the gate passes.

## Evidence contract

Each platform task is one command with no dependency on another platform's
artifacts:

1. Assert the host OS/architecture and exact pinned Wails CLI.
2. Build fresh production frontend assets and a clean, trimmed Wails binary.
3. Validate the native executable architecture and package artifact.
4. Install installer-based distributions into an isolated user-owned location
   and validate their registration, shortcuts, and installed payload.
5. Launch the installed or packaged executable twice and require a committed
   React render, one start/clean-stop lifecycle pair per launch, and no external
   sidecar.
6. Reopen the same SQLite database and require the smoke story and shelves.
7. Run the native multi-file, directory, and reveal-adapter acceptance tests.
8. Run the complete headless composition and root release contracts.
9. Uninstall installer-based distributions and prove application files are
   removed without deleting user-owned SQLite data.
10. Verify the distribution artifact's SHA-256 checksum.

The implementation is:

- macOS: `scripts/verify-packaged-acceptance`;
- Windows: `scripts/verify-platform-windows.ps1`;
- Linux: `scripts/verify-platform-linux`; and
- CI orchestration: `.github/workflows/platform-release.yml`.

GitHub Actions uploads only artifacts produced after the complete platform task
passes. Failed or cancelled jobs publish no qualified release artifact.
