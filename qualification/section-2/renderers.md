# Renderer/runtime gate (2.7, P56–P57/P59)

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
