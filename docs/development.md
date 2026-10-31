# Native Rust development

These instructions describe `rewrite-in-rust-native-desktop`. The Rust candidate
uses Slint/Winit/Skia and bundled SQLite. The retained Go/Wails application and
its checks remain available until the release and cutover gates pass. Building
Rust does not require Node, Wails or a browser. Installed native packages need
neither Rust nor Go; see the [platform matrix](release-platforms.md).

## Build hosts and tools

Use macOS 15 or later on Apple silicon or Intel, or Ubuntu 24.04 x86-64. Build
each distribution on its matching host; cross-compilation and Rosetta do not
replace native qualification. Other distributions, architectures and Windows
are outside the current qualified matrix.

| Tool | Source of truth / purpose |
| --- | --- |
| Rust and Cargo through rustup | Rust **1.99.1**, edition 2024 and resolver 3; [rust-toolchain.toml](../rust-toolchain.toml) selects rustfmt, Clippy and llvm-tools-preview. |
| Python 3 | Requires the standard-library `tomllib` and `sqlite3` modules for source, preservation and packaging checks. |
| Native compiler/linker | macOS: Xcode/Command Line Tools with the macOS SDK and Clang; native observer qualification also compiles Swift helpers. Linux: the compiler packages below. |
| Routine Cargo helpers | `cargo xtask tools` installs the exact locked nextest/deny/audit versions in [.config/rust-tools.toml](../.config/rust-tools.toml). |
| Go | **1.26.5** for the separate Go/native upgrade and storage-handover rehearsals in CI. It is not linked into the Rust application. |

On macOS, use the deployment baseline when building from source:

```sh
export MACOSX_DEPLOYMENT_TARGET=15.0
```

On Ubuntu 24.04, install the native build prerequisites used by the package
builder. Runtime dependencies and a graphical desktop are listed separately in
the [platform matrix](release-platforms.md#linux-runtime-prerequisites).

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config python3 clang cmake ninja-build libfontconfig1-dev libfreetype6-dev libx11-dev libx11-xcb-dev libxcursor-dev libxi-dev libxrandr-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev libdbus-1-dev desktop-file-utils
```

Build and check from the repository root:

```sh
rustup show
python3 -c 'import sqlite3, tomllib'
cargo xtask tools
cargo build --locked --package librairii-desktop
cargo xtask check
```

The initial build downloads the locked Cargo/native dependency inputs. Checks
also need access to their advisory databases. Keep `Cargo.lock` and the reviewed
derivatives in `third_party/` intact; do not regenerate the frozen Go oracle or
accept a changed fixture to make a check pass.

The maintained feature combinations are the three native defaults in
[.config/rust-checks.toml](../.config/rust-checks.toml). Do not substitute
`--all-features`: renderer and graph changes require their own qualification.

## Run a disposable development library

This command opens the native GUI. Run it only on a desktop intended for manual
development, using synthetic archives and a fresh empty root:

```sh
librairii_demo_root="$(mktemp -d)"
LIBRAIRII_DATA_ROOT="$librairii_demo_root" cargo run --locked --package librairii-desktop
```

The override bypasses normal profile/default-library discovery and starts in
English. It accepts only an absolute fresh directory and disables relocation;
the populated directory is retained after exit for inspection. Use a new empty
directory for another override launch. Do not point it at an existing library.
Normal reopen, settings persistence, adoption and relocation are exercised by
the separate isolated packaged-acceptance route.

The data-root override does not redirect Lunii networking. Do not request live
sync in automated fixtures; network tests use explicit numeric-loopback clients.
Installed GUI/input/accessibility qualification runs on disposable native CI
desktops, with its own validated library/profile/catalog scope. It must not run
against a contributor's normal profile or library.

## Checks and editor setup

| Command | What it verifies |
| --- | --- |
| `cargo xtask check` | Native host/pins, module boundaries, frozen preservation corpus, source/dependency/license/advisory policy, formatting, locked build, Clippy with warnings denied, test discovery, nextest and separate doctests. |
| `cargo nextest run --workspace --locked --profile ci` | Workspace unit/application/integration tests; JUnit goes to `target/nextest/ci/junit.xml`. |
| `cargo test --workspace --doc --locked` | Doctests, which nextest does not execute. |
| `cargo fmt --all -- --check` | Workspace formatting. |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Host compilation and lint policy. |
| `cargo xtask inventory` | Resolved dependency inventory; the release SBOM separately uses actual compiler artifacts. |
| `cargo xtask coverage` | Workspace coverage with the pinned llvm-cov/nextest helpers. |
| `cargo xtask bench --smoke` | The focused benchmark harness; measured scale gates run separately in native CI. |
| `cargo xtask fuzz` | Bounded retained fuzz corpora using the exact dated toolchain in [fuzz/rust-toolchain.toml](../fuzz/rust-toolchain.toml). |

`cargo xtask tools` installs the three routine helpers only. Install the exact
`cargo-llvm-cov`/`cargo-fuzz` versions from `.config/rust-tools.toml` and the dated
fuzz toolchain before using the optional focused commands. Ordinary builds use
the stable pin.

Open the workspace root in an editor with Rust Analyzer. Use the rustup compiler,
set `rust-analyzer.check.command` to `clippy`, enable
`rust-analyzer.cargo.allTargets`, and leave `rust-analyzer.cargo.allFeatures`
disabled. `cargo fmt` supplies formatting; no editor extension is required for
the command-line build.

The three native CI jobs add actual platform observations, isolated Go/native
handover and crash/filesystem rehearsals. Passing a local host check does not
qualify another host, installed GUI, screen-reader output or public signing.
See [CI policy](rewrite/ci.md), [scale evidence](rewrite/section-16.md) and
[package qualification](rewrite/section-18.md).

## Build distribution candidates

The [manual build and signing guide](native-build-and-sign.md) provides a
single-command package helper and a signing-command preview. You choose when
to execute a build, provision signing credentials and approve signing.

Use a clean committed checkout on the matching native host. Each output must
be a new directory below `target/native-release`; choose unused names for repeat
runs. The release profile and embedded source/graph identity are checked by the
builder, rather than inferred from an arbitrary `cargo build` output.

```sh
librairii_target="$(rustc -vV | sed -n 's/^host: //p')"
cargo xtask release plan --target "$librairii_target"
cargo xtask release build --target "$librairii_target" --output target/native-release/local-stage
cargo xtask release verify-stage target/native-release/local-stage
```

`plan` reports the selected inputs without building or qualifying a package.
`build` produces a sealed payload containing the executable, embedded identity,
icons, SPDX inventory and notices. Use the matching assembler:

```sh
# On macOS, with the matching ARM or Intel stage:
cargo xtask release package-macos --stage target/native-release/local-stage --output target/native-release/local-package

# On Ubuntu 24.04 x86-64 instead:
cargo xtask release package-linux --stage target/native-release/local-stage --output target/native-release/local-package
```

These commands create internal candidates. The Mac DMG is explicitly unsigned
for public distribution despite its ad-hoc integrity signature. Final Developer
ID signing, notarization, stapling and credentialless installed-byte checks use
the [protected signing workflow](rewrite/section-18.md#protected-mac-signing).
Artifact/checksum records identify exact package bytes; inspection and GUI
qualification remain separate gates.

`cargo xtask release inspect-package` checks installation, linkage and embedded
identity without a GUI. `cargo xtask release qualify-gui` exercises the installed
product and is restricted to disposable native CI. The complete invocation and
evidence contract are in [task 18](rewrite/section-18.md). Package installation
requires no source checkout or developer toolchain.

## Retained Go compatibility checks

`make check`, `make build`, `make dmg`, `make linux` and the old packaged
acceptance commands still belong to the Go/Wails application. They do not build
or qualify the Rust candidate. Retain them until the separately reviewed runtime
retirement; Go is also the frozen behavioral oracle and handover source.

For changes covered by the legacy CI selector, use Go, the Node version in
[.nvmrc](../.nvmrc), the Wails pin in [.wails-version](../.wails-version), and
Chrome/Chromium for browser acceptance. `make setup` installs the retained build
dependencies; `make check` runs the legacy checks. Linux legacy builds additionally
use GTK 3/WebKitGTK 4.1 development packages and `GOFLAGS="-tags=webkit2_41"`.
Those browser libraries are not dependencies of the native Rust package.

The older frontend screenshot command captures the Go UI. Its retained images
and [capture notes](assets/README.md) are historical visual references rather
than screenshots of the native application.
