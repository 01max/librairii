# Host and SDK inventory (task 2.2)

## Current availability correction (2026-10-23)

The owner clarified that they have neither an Intel Mac nor a Linux desktop.
This supersedes the earlier statement that the owner could run both sets of
checks. No interactive operator or host is secured for either platform; do not
assign those checks to the owner or infer access from the package downloads.

The development Apple-silicon Mac remains available. Hosted CI has now actually
built and exercised the native ARM macOS, Intel macOS and Linux x86-64 probes,
including packaged macOS smokes and all three immediate runtime audits. Linux
X11/Xvfb GPU-API initialization failure and multilingual Skia CPU recovery pass
in [run 39956378956](https://github.com/01max/librairii/actions/runs/39956378956)
at implementation revision `96f6222`. This is automated hosted execution; it
does not provide an interactive Intel/Linux operator, physical input/IME,
mixed-monitor, real Wayland desktop, Orca or clean-host qualification.

Task 2.2 and the affected interactive gates remain incomplete. Unavailable
hosts are not a reason to stop independent measurements, package work or
additional automated Linux session coverage.
VoiceOver remains skipped by the separate explicit owner decision; Windows
remains deferred. The original inventory below is historical, not current state.

## Historical initial inventory

Observed 2026-10-20 by librairii-4 through local CLI and read-only GitHub API.
No resources provisioned, VM started, credentials printed or paid runner used.

| Resource | Observed availability | Qualification still required |
| --- | --- | --- |
| Apple silicon | Apple M4 Pro, 48 GiB, Darwin arm64, macOS 15.7.7 build 24G720 | GUI execution/VoiceOver, physical input/IME/mixed monitors, clean user/package |
| Apple SDK | CLT `/Library/Developer/CommandLineTools`, SDK 26.2; SDK 15.2/15.4 also present; Apple clang 17.0.0 | Full Xcode absent from Applications; `xcodebuild -version` rejects CLT selection. Signing qualification pending |
| Rust | Existing stable 1.97.1 via `rustup run stable`; cargo/rustc absent from original PATH. Candidate 1.99.1 installed into task-specific `/private/tmp` rustup/cache | Other target standard libraries/OS toolchains not inventoried on remote hosts |
| Intel Mac | No identified accessible real Intel Mac | Named real Intel macOS 15+ host, SDK, interactive desktop/VoiceOver, operator/access |
| Linux desktop | No identified x86-64 Ubuntu 24.04 desktop | Both X11 and Wayland sessions, portal backend, file manager, AT-SPI/Orca, fonts/IME, GPU+CPU path, clean package host |
| Windows | No identified Windows 11 x64 execution host | Windows 11 x64 VM/hardware, MSVC Build Tools C++/Windows SDK, NVDA, IME/DPI, GPU/remote software session, clean installer host |
| Local virtualizers | Docker contexts default/desktop-linux and two unrelated running containers; UTM application present | Existing workloads untouched. `utmctl list` reports AppleEvents -1743; no VM inventory established. ARM virtualization is not Intel Mac or x64 native qualification |
| Repo CI | `GET /repos/01max/librairii/actions/runners`: total_count 0 | No self-hosted execution hosts registered. Hosted availability/budget/actual run separate |
| Existing workflows | Go/Wails `ci.yml`: macos-15; platform-release.yml: ubuntu-24.04 | No Rust qualification, Windows or Intel GUI/AT lane. Existing Xvfb is not Wayland/Orca/physical desktop evidence |

Exact commands: `uname -sm`, `sw_vers`, `sysctl -n machdep.cpu.brand_string`,
`sysctl -n hw.memsize`, `xcode-select -p`, `xcrun --sdk macosx --show-sdk-version`,
`clang --version`, `rustup toolchain list`, `rustup target list --installed`,
`rustup run stable rustc --version --verbose`, `xcodebuild -version`,
`docker context ls`, `docker ps`, `utmctl list`, and the scoped runners API above.
No SSH config, VM disks, keychains, `.env` or production profile was inspected.

## Baselines proposed by the change, not established support

* ARM and Intel macOS 15+, deployment target 15.0; use matching SDK builders.
* Linux x64 Ubuntu 24.04 LTS, glibc 2.39 distribution baseline. Pin builder image
  and inspect actual ELF symbol versions before claiming a minimum. X11 requires
  libX11/X11-xcb/Xcursor/xkbcommon-x11; Wayland/client/xkbcommon and portal+desktop
  backend/DBus, fonts/fontconfig/Freetype and graphics libraries depend on the
  final graph. `ldd`/`readelf` and clean desktop execution must settle the list.
* Windows 11 x64 MSVC; record exact compiler/SDK image after access. Skia adds
  a C++ runtime; `dumpbin /dependents` and clean launch determine redistribution.

[Slint native dependencies](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/),
[Windows runtime](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/windows/packaging/).

GitHub's current [runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
lists `macos-15` (ARM), `macos-15-intel`, `ubuntu-24.04` (x64) and
`windows-2025` (x64 Server). These are prospective build resources, not observed
execution availability. Server is not Windows 11. Hosted Intel is not evidence
of interactive real-hardware VoiceOver qualification. Private-repo runner billing
must be resolved before provisioning anything outside the existing workflow.

## Resource request delivered early

`ao send` to librairii-1 requested named Intel, Linux desktop and Windows hosts,
interactive access/operators for screen readers, keyboard layouts/IME, mixed-DPI
monitors and clean user/package execution. ARM interactive access remains to be
observed. UTM automation access is a precise inventory limitation, not proof of
missing VMs. Signing account/certificate access is a later release gate and does
not block unsigned independent work or eligible sections 3–4.

**2.2 remains incomplete:** hosts have not all been secured.

Follow-up inventory: `xcrun --find notarytool` and `xcrun --find stapler` both
resolve under CLT; `hdiutil`, `ditto`, `codesign` are present. Full Xcode is not
required merely to attempt unsigned packaging here. No signing identity/keychain
or secret value was queried. Computer Use rejected access to the distinct
`dev.librairii.qualification` app; ARM keyboard/AX/visual observations need app
access approval or an operator. These exact gaps were delivered to librairii-1.
