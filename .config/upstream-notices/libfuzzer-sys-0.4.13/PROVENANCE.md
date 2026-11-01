Fuzz-only upstream native runtime notices (not a Librairii license).

libfuzzer-sys0.4.13 declares `(MIT OR Apache-2.0) AND NCSA`; its README calls
libfuzzer NCSA, while bundled source headers identify Apache-2.0 WITH LLVM-exception.
Keep the complete upstream LLVM/compiler-rt license, including historical notices,
rather than assume the Rust wrapper license covers the C++ implementation.

Published .cargo_vcs_info.json identifies rust-fuzz/libfuzzer commit
719e4efb9b8857ebaa782ae59376c8cbb78fed0f. Its update-libfuzzer.sh pins LLVM commit
a47b42eb9f9b302167b4fc413e6c92798d65dd0b. Retrieved2026-10-20:

- https://github.com/rust-fuzz/libfuzzer/blob/719e4efb9b8857ebaa782ae59376c8cbb78fed0f/update-libfuzzer.sh
- https://github.com/llvm/llvm-project/blob/a47b42eb9f9b302167b4fc413e6c92798d65dd0b/llvm/LICENSE.TXT
- https://github.com/llvm/llvm-project/blob/a47b42eb9f9b302167b4fc413e6c92798d65dd0b/compiler-rt/LICENSE.TXT

No fuzz runtime is shipped in the desktop skeleton. Release inventories must
separately evaluate any future production native/asset graph.
