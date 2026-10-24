# Native notice supplement (2.13)

The Rust registry notice inventory did not cover Skia's bundled native source
graph. The pinned ARM `libskia.a` archive contains codec, XML, color-management
and compression objects. Archive membership alone does not prove linkage into
the executable; this supplement deliberately includes a conservative union.
Linux's selected Vulkan graph additionally motivates VMA and Khronos notices.

`notices/native-skia/inventory.json` records the Skia source ref/tree, DEPS blob,
individual pinned revisions, upstream notice paths and retained file hashes.
The original texts cover Skia, skcms, libpng, libjpeg-turbo/IJG, zlib/Chromium
extensions, Expat, Wuffs, Vulkan Memory Allocator and Khronos Vulkan headers.
The IJG binary acknowledgment is in `ATTRIBUTIONS.md`. The separate skcms BSD
text is retained verbatim. Chromium's generic BSD text has its own immutable
blob identity; it is not misattributed to the zlib commit. Per-file Chromium,
ARM and Intel copyrights are preserved. The actual Khronos header SPDX terms
determine its notice; the neighboring Google BSD wrapper is not substituted.

`native-notices.py` refuses a changed binding version/Skia source ref, missing
component/file/acknowledgment, changed or aliased notice, or mismatching native
build license. It validates the entire supplement before copying. Packaging
includes it alongside Rust notices and inside macOS Resources before sealing
and hashing the completed bundle. The manifest identifies the inventory and
retains `final_native_distribution_audit_complete=false`.

Four tests cover exact copied bytes and failures before copying for altered,
missing or symlinked notices; a changed graph or native build notice; and a
rehashed missing IJG acknowledgment/component. A real offline Cargo metadata
check matches binding0.153.3/Skia m153-0.101.2 and both available local native
license outputs. These checks do not replace fresh three-target packaged-byte
verification and independent final distribution review. Task2.13 and final
toolkit selection remain open.

2026-10-24 Task2.13 review correction: compiled SkSL includes the pinned SPIR-V
and GLSL.std.450 headers. Their exact common Khronos2014–2016 notice is now
packaged separately from the Vulkan Apache terms. Both immutable Git blob
identities and the notice digest are in the native inventory. No selected
Linux/macOS graph basis was found for Windows/D3D SPIRV-Cross.
