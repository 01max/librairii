# Native platform matrix

This page describes the Rust candidate on `rewrite-in-rust-native-desktop`.
The three native targets have installed-package CI evidence. Public Mac signing,
final signed-byte verification and release promotion remain open gates; an
unsigned candidate is not a signed public release. The retained Go/Wails
application and its build commands are described in
[legacy compatibility checks](development.md#retained-go-compatibility-checks).

## Tested targets and artifacts

| Native target | Baseline | Candidate artifact | Qualification |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | macOS 15+, Apple silicon | Thin `Librairii.app` in `Librairii-<VERSION>-aarch64-apple-darwin-unsigned.dmg`, checksum | Matching ARM Mac runner; unsigned installed GUI passes. Developer ID/notarization/stapling and final signed-byte gates remain open. |
| `x86_64-apple-darwin` | macOS 15+, Intel | Separate thin `Librairii.app` in `Librairii-<VERSION>-x86_64-apple-darwin-unsigned.dmg`, checksum | Native Intel hardware; unsigned installed GUI passes. Rosetta is supplemental evidence only; signed-byte gates remain open. |
| `x86_64-unknown-linux-gnu` | Ubuntu 24.04 LTS x86-64, glibc 2.39 | `Librairii-<VERSION>-x86_64-unknown-linux-gnu.tar.gz`, executable, desktop entry/icons, checksum | Installed native GUI passes in X11 and Wayland, with GPU-API and software-rendering cases. |

The latest integrated implementation was checked as PR 69 head
`21f32ed968cd4e66bfde6817e0f77b4c93dac810`:
[native packages/installed GUI](https://github.com/01max/librairii/actions/runs/41185064509),
[quality/storage/handover](https://github.com/01max/librairii/actions/runs/41185064457),
[scale/resources](https://github.com/01max/librairii/actions/runs/41185064367)
and [native foundations](https://github.com/01max/librairii/actions/runs/41185064444)
pass on all three targets. These links identify their recorded revision; they
do not qualify a later rebuild or its new bytes.

macOS executables declare deployment target 15.0; the evidence records the actual
OS, SDK, architecture and hardware used. The Linux build and runtime baseline
is Ubuntu 24.04/glibc 2.39, not every distribution with a Linux kernel. Other
distributions/OS baselines, Linux ARM64, universal Mac bundles and Windows need
separate qualification. Existing Windows source/history does not establish
production support.

## What runs on the user's machine

The application is a Rust executable with compiled Slint/Winit/Skia views,
embedded English/French catalogs, product icons/artwork fallbacks and bundled
SQLite. It does not require a source checkout, Cargo/Rust, Go, Node, Wails,
Chrome or WebKit to run. Native OS graphics, fonts, dialogs, accessibility and
file-manager services remain system dependencies. See the
[current architecture](architecture/0005-rust-native-desktop.md).

On macOS, AppKit, Foundation, CoreGraphics, Metal and the system graphics/font
services come from macOS. Use the bundle for the machine's actual architecture.
Public distribution must complete the
[protected signing and verification workflow](rewrite/section-18.md#protected-mac-signing)
before signed-installation support is claimed.

## Linux runtime prerequisites

Run inside an X11 or Wayland desktop with a D-Bus session, a working
file-chooser portal, installed fonts and an `inode/directory` file-manager
handler. CI uses the GTK portal and Thunar; its actual file-manager location
and window retirement are observed. A headless server or container without
these desktop services is not a qualified interactive host.

The package's `share/librairii/runtime-requirements.json` records these declared
Ubuntu runtime packages:

```sh
sudo apt-get install -y ca-certificates libgcc-s1 libstdc++6 libfontconfig1 libfreetype6 libx11-6 libx11-xcb1 libxcursor1 libxi6 libxrandr2 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libegl1 libgl1 libvulkan1 mesa-vulkan-drivers libdbus-1-3 xdg-desktop-portal xdg-desktop-portal-gtk libglib2.0-bin
```

The qualified image additionally installs `libwayland-cursor0`,
`libwayland-egl1`, `fonts-dejavu-core`, `fonts-noto-cjk` and `thunar` for its
concrete desktop/font configuration. Preserve those requirements when
reproducing that environment. Driver/GPU and font differences require their own
observations rather than a blanket compatibility claim. The exact image is in
[qualification/section-18/Dockerfile](../qualification/section-18/Dockerfile).

`ca-certificates` is required even for the local-fixture qualification route:
the HTTP client's platform trust store is initialized before requests. Ordinary
library workflows work offline. Catalog sync and uncached artwork use the
documented optional Lunii network boundary.

The default Skia backend uses the installed graphics stack. To select the
qualified CPU rendering route when launching from an extracted package:

```sh
SLINT_BACKEND=winit-skia-software ./bin/librairii-desktop
```

Keep the listed desktop libraries available for that route too. The native
binary does not use WebKitGTK; GTK libraries used by the portal and browser
libraries retained for Go development/CI are separate from its runtime.

Linux spoken-accessibility qualification uses AT-SPI and Orca. Those services
and the screen reader must be available for that use. Python AT-SPI bindings,
Xvfb/Openbox, Sway, input/screenshot observers and the Go compatibility helpers
in CI are qualification tools, not shipped product runtimes. Mac AX observations
do not claim VoiceOver execution; VoiceOver and script-specific CJK/Arabic
input/IME/shaping qualification were excluded/deferred by the owner. Valid
UTF-8 data and filenames retain their lossless behavior.

## Install and retain data

Verify the checksum belonging to the exact package before installation.
The Mac artifact contains an application bundle in a read-only DMG. Signing
records distinguish an internal ad-hoc candidate from a Developer ID-signed,
notarized and stapled distribution; a build or ad-hoc integrity pass cannot
stand in for those public-release checks.

For Linux, extract the versioned archive into a permanent application directory
and launch `bin/librairii-desktop`. It contains `bin/`,
`share/applications/io.github.librairii.native.desktop`, hicolor icons,
notices/SPDX and runtime/linkage records. For desktop integration, install its
`bin/` and `share/` into the same chosen prefix and make that prefix's `bin`
available in the desktop session's PATH and its `share` in XDG_DATA_DIRS.
The desktop entry's command is `librairii-desktop`; it must resolve to this
installation. See the archive's `INSTALL.txt`.

Updates/uninstall replace or remove application files and their explicitly
installed desktop/icon entries. Keep the independent native profile, library,
backups and recovery journals. Reinstall can reopen the retained library;
removing an app directory is not permission to remove user data. Existing Go
libraries require the [verified transition procedure](native-transition.md)
rather than manually directing an older executable at the native candidate.

## Evidence for each candidate

The [release builder](rewrite/section-18.md) records source commit/tree,
product version, target, features, locked/compiler graph, production profile,
notices/SPDX and final artifact SHA-256. `cargo xtask release inspect-package`
installs/extracts the selected bytes into a fresh location, compares payload
bytes/modes, checks architecture/linkage and runs the installed identity command
without opening a GUI.

A separate disposable native CI job runs the actual installed production GUI.
Both Mac targets cover fresh English and upgraded French fixtures; Linux repeats
them under X11 and Wayland. Each lane requires:

- Native picker, menu/accelerator and file-manager behavior, including exact
  destination and positive file-manager-window closure.
- Import/inspection, metadata/artwork, tags/search, Collections, approved
  Shelves/review, Broken/Undo, all export scopes, trash and durable History.
- Exact managed/exported bytes, memberships and persisted database state.
- GPU-API/software routes, scaling, offline reopen, settings/language,
  relocation, real in-flight SIGKILL/recovery and application-only reinstall.
- Positive product/reader/container/cache retirement with raw observations,
  screenshots and source/method bindings retained against the package checksum.

The final signed Mac bytes must repeat applicable inspection and GUI checks in
a credentialless job after signing/notarization/stapling. Keep failed, skipped
and unavailable gates visible. Build success, cross-compilation, a source-host
launch or an earlier unsigned artifact cannot qualify that distribution.

For build-host dependencies and exact commands, see
[native development](development.md). The CI entry points are
[Rust packages](../.github/workflows/rust-packages.yml),
[Rust quality](../.github/workflows/rust-quality.yml) and
[protected Mac signing](../.github/workflows/rust-sign-macos.yml).
