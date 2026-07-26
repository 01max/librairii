# Release platform matrix

Status date: 2026-07-26

Release qualification runs on the target operating system in an interactive
desktop session and invokes its host-native platform services. A
cross-compiled binary, a frontend-only test, a successful installer build, or
a hosted runner without working desktop WebView support does not by itself
qualify a target. Every qualified row must independently build its
distribution artifact, launch the actual packaged executable twice, commit the
embedded React render, create and reopen SQLite state, pass host-native dialog
and reveal acceptance, run the complete headless story-library smoke, and
produce a verified SHA-256 checksum.
The packaged acceptance driver opens the production platform dialogs, selects
only the prevalidated isolated fixture paths through a host-native automation
hook, validates the paths returned by Wails, and records separate native
import, destination, and reveal evidence before domain work may continue.

## Priorities and artifacts

| Priority | Distribution target | Verification host | Artifact | Command | Status |
| --- | --- | --- | --- | --- | --- |
| 0 | macOS 15, arm64 | macOS 15.7.7 arm64 | versioned DMG | `make verify-packaged-acceptance` | Passed on the current host |
| 1 | Windows x64 | Interactive Windows 11 x64, or Windows 11 arm64 with x64 emulation | per-user NSIS installer | `make verify-platform-windows` | Unqualified: command defined; awaiting an interactive-host pass |
| 2 | Linux x64 with WebKitGTK 4.1 | GitHub-hosted Ubuntu 24.04 x64 under Xvfb | versioned portable `tar.gz` | `make verify-platform-linux` | Defined as a host-native CI gate |

Windows is first after macOS because it adds the most distinct packaging and
runtime boundary: PE/NSIS, WebView2, Explorer, and Windows native dialogs.
Ubuntu Linux follows to cover ELF/dynamic dependency validation, WebKitGTK,
XDG storage, `xdg-open`, and headless desktop launch. macOS Intel/universal,
Windows arm64, and Linux arm64 are not release targets for `0.1.x`; adding one
requires a new matrix row and the same evidence.

The Windows installer is built with user install scope and expects a supported
WebView2 runtime. The full qualification command checks Microsoft's machine
and current-user WebView2 registry keys before acceptance. When neither
contains a usable version, it downloads the official Evergreen bootstrapper,
verifies its Microsoft signature, installs it silently, and requires the
runtime to appear in the registry before continuing. It installs Librairii
into an isolated user-owned directory, validates its current-user uninstall
registration and both shortcuts, and runs acceptance from the installed copy.
It then uninstalls and requires the install files, shortcuts, and registration
to be gone while the SQLite library remains. Its packaged executable is
verified as an amd64 PE image using the Windows GUI subsystem, so it does not
open an extra console window.

The named hosted entry point is `make verify-platform-windows-hosted`; GitHub
Actions invokes its underlying script with `-ArtifactOnly`. It builds the
production installer with Wails' standard Go WebView2 loader, checks its PE
architecture and GUI subsystem, installs it, validates payload hashes,
shortcuts and registration, runs the complete headless Go/SQLite and
platform-adapter smoke as Windows amd64, uninstalls it, proves user data
retention, and verifies the installer checksum. The uploaded artifact is named
as a candidate and is not release-qualified. Both the standard Wails 2.13 Go
loader and its documented `native_webview2loader` fallback failed before
application `OnStartup` on GitHub-hosted Windows; the fallback is therefore not
shipped. Full render, native-dialog, reveal, relaunch, and clean-shutdown
evidence remains mandatory on an interactive Windows host.

The checked-in NSIS project replaces Wails 2.13's native-only architecture and
payload macros because those macros reject an amd64-only installer on every
arm64 host. The replacement still rejects Windows 10 on Arm, which cannot
emulate x64 applications, and admits the amd64 payload on arm64 only when NSIS
confirms Windows 11 or later. The hosted Windows 11 arm64 job runs that exact
amd64 installer through Windows x64 emulation; it does not produce or qualify a
Windows arm64 artifact.
The Linux archive contains the Wails executable and expects the distribution
to provide GTK 3 and WebKitGTK 4.1. It is intentionally not a generic package
for every Linux ABI.

The Linux gate runs the packaged scenario in one isolated D-Bus/Xvfb desktop
session. It registers a verification-only `inode/directory` handler that
delegates to PCManFM, then requires the live PCManFM process and its command
line to contain the exact validated export directory before the gate passes.

## Evidence contract

Each platform task is one command with no dependency on another platform's
artifacts:

1. Assert the host OS/architecture and exact pinned Wails CLI, and provision
   the platform web runtime when the host image does not supply it.
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

The qualification implementation is:

- macOS: `scripts/verify-packaged-acceptance`;
- Windows WebView2 preflight: `scripts/ensure-webview2-runtime.ps1`;
- Windows: `scripts/verify-platform-windows.ps1`;
- Linux: `scripts/verify-platform-linux`; and
- CI orchestration: `.github/workflows/platform-release.yml`.

The same Windows script accepts `-ArtifactOnly` for hosted candidate evidence;
`make verify-platform-windows-hosted` is the named entry point. GitHub Actions
labels and uploads that installer only as
`Librairii-windows-amd64-candidate`. No Windows job or artifact is called
qualified until the default script mode passes on an interactive Windows host.
Failed or cancelled jobs publish no artifact.
