# Renderer/runtime gate (2.7, P56–P57/P59)

## Forced Linux GPU initialization failure probe (2026-10-23)

`check-gpu-recovery.py` runs the archive-round-tripped, manifest-verified Linux
package as an ordinary user. A working OpenGL control must report its actual
OpenGL surface. Failure cases start a separate Xvfb X11 server with GLX disabled
and point EGL/Vulkan driver discovery at an intentionally absent manifest. These
are child-process overrides; no installed driver, system setting or package is
changed. The loader mechanisms are documented by
[libglvnd](https://github.com/NVIDIA/libglvnd/blob/master/src/EGL/icd_enumeration.md)
and the [Vulkan loader](https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderDriverInterface.md).

Separate public `BackendSelector::require_opengl` and `winit-skia-wgpu` controls
must reject initialization under injection. The ordinary `winit-skia` request,
with the same injection, must
emit both GPU-candidate failures and report the actual `skia backend software`
surface through Slint's performance diagnostics. The pinned Slint 1.18.1
`create_default_surface` already tries WGPU, OpenGL and software candidates;
this probe adds evidence rather than an application retry policy. A missing
tool, signal, timeout, successful GPU-only control, absent fallback diagnostics
or failed component assertion cannot qualify recovery. Each process group is
bounded to 30 seconds and reaped on timeout.

The public `--renderer-smoke=NEW_DIRECTORY` route saves a French/CJK scene,
dispatches Search input to the actual widget, checks the filtered model and
selection, and saves a changed frame. Normal owned shutdown still joins workers
and retires fixtures. `Window::take_snapshot` re-renders on CPU; its pixels do
not identify the on-screen surface. The independent backend trace supplies that
identity. CI retains command outputs, source/executable/frame hashes, driver
versions, Xvfb session description and both fresh PNG frames for each successful
case. Visual glyph review remains separate from non-flat/changed-pixel checks;
component input is not physical input, clipboard or IME evidence. Xvfb does not
qualify a real Wayland/remote desktop or recovery from GPU loss after startup.
The first Linux run
[39955558369](https://github.com/01max/librairii/actions/runs/39955558369), head
`5ab2ec9`, retained a failing overall verdict: recovery itself reported both GPU
errors and the software surface, with successful rendering/input, but the
`skia-opengl` alias was unrecognized in this feature graph and fell back silently.
The corrected control uses the public API requirement without changing the graph.
That run also demonstrated that CPU snapshots can execute before a queued GPU
initialization error returns. Failure controls now use the bounded ordinary
smoke rather than snapshots. The report and original failure remain retained;
this first run does not qualify the control comparison. Corrected Linux results
remain pending until the next revision executes in CI.

## Current runtime inspection (2026-10-23)

The current scope is ARM macOS, native Intel macOS and Linux x86-64; Windows
below is historical. `inspect-runtime.py` inspects the actual packaged binary
after archive byte/native-host verification. It records source and executable
hash, raw native-tool outputs, immediate imports and loader paths. macOS records
both Mach-O minimum-OS commands and the verified bundle's declared minimum;
Linux records ELF needed sonames, actual `ldd` resolution and numeric
GLIBC/GLIBCXX/CXXABI symbol requirements, preserving nonnumeric ABI names too.
Failed tools, timeouts and unresolved Linux/non-system macOS imports fail the
inspection and retain their available outputs. Run only against trusted probes;
`ldd` can invoke the binary's loader. CI retains `runtime-requirements.json` for
each of the three native targets. All three passed in CI run
[39950896230](https://github.com/01max/librairii/actions/runs/39950896230) at PR head
`9dc3fca` (built merge `8317ce85854cef0fce245384d6941f09be37b53f`): macOS release
packages have 18 OS-only imports and both minima 15.0; Linux resolves all seven
immediate sonames with maximum GLIBC 2.39, GLIBCXX 3.4.26 and CXXABI 1.3.9.
These requirements do not establish the full minimum supported desktop baseline.

The clean local ARM package from `af132fe` passed immediate import inspection:
18 imports, all OS libraries/frameworks, no rpaths. Its Mach-O declares 11.0,
while its app bundle requires 15.0. That discrepancy is recorded explicitly;
neither value proves older-OS execution or changes the proposed macOS 15 baseline.
The executable hash is `b3322941ad1d35df9ccc25c60cff0d7dff75a7e99e2c634fe992e30e78d90529`.
Actual GPU-failure recovery, libraries loaded dynamically at runtime, desktop
plugins and clean-host availability remain separate open checks. This inspection
does not select a renderer or toolkit or close all of 2.7.

## Historical initial ARM probe

Observer librairii-4, 2026-10-20, ARM macOS 15.7.7/M4 Pro, Rust 1.99.1.
The initial locked GUI build uses Slint 1.18.1, Winit 0.30.13,
skia-safe/skia-bindings 0.153.3, AccessKit 0.24.1 and accesskit_winit 0.33.2.

Observed process runs (debug build, three-second self-close):

| Invocation | Result | Scope |
| --- | --- | --- |
| `SLINT_BACKEND=winit-skia qualification-desktop --smoke` | exit 0, construction 160 ms | Normal backend requested; no GPU trace or visual inspection, so active GPU not certified |
| `SLINT_BACKEND=winit-skia-software qualification-desktop --smoke` | exit 0, construction 133 ms | Explicit Skia CPU requested on ARM; glyph correctness/IME not observed |

`file` reports Mach-O arm64. `otool -L` reports only OS libSystem, libc++, libobjc,
libiconv and AppKit/Foundation/CoreFoundation/CoreGraphics/CoreText/CoreVideo/
ApplicationServices/Metal/MetalKit/OpenGL/Carbon/QuartzCore/ColorSync frameworks.
No WebKit/Chromium/Go dylib import was found. This is immediate binary linkage
inspection, not a complete process/resource audit or all-target result.

The explicit CPU request is **not forced GPU initialization failure**. No Linux
VM/Windows remote software environment, GPU failure injection or multilingual
rendered output was observed. The different lightweight `winit-software` renderer
is not used as evidence. Keep 2.7 open until Linux and Windows failures fall back
correctly with French/CJK shaping and interaction, and all four target linkage
inventories exist. Record target/session/driver, failure method, selected renderer,
package hash, native library versions and actual screenshots/AT/IME results.
