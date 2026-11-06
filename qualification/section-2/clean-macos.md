# Native macOS clean-runtime qualification (Task2.11)

`check-clean-macos.py` verifies every packaged member and matching archive, copies
it to an owned isolated installation, and launches/relaunches it twice on each
normal/explicitCPU backend on the matching ARM or real Intel CI host. Each launch
must expose an actual owned CoreGraphics window, produce two distinct rendered
French/UTF-8 frames, perform the public component Search/model update, exit0 and
join workers/retire its fixture. Actual native window images and raw identities
are retained. The isolated installation and fresh HOME/TMPDIR are then removed.

The child receives a fresh environment without Cargo/Rust/development variables.
An inherited process sandbox denies network, source/cache/development-runtime
reads and all executable launches except the exact app and named negative-control
system utilities. Actual source-read/compiler-execution denial and a denied
loopback connection to an owned listening port are required; a positive system
read and an unsandboxed connection prove the controls would otherwise work.
Crashes, missing files, capture errors or timeouts cannot count as denial.

This proves the spike's runtime independence on the identified CI OS. It does
not claim an erased macOS installation, minimum-version execution, Gatekeeper
acceptance, notarization or a finished product installer. Task18 owns public
release signing/installer lifecycle; existing signing credential requirements
remain documented. The minimal selected package is the sealed `.app` inside the
verified tar archive; the optional DMG experiment is separate. Native execution
and independent evidence review pass on both ARM and real Intel. Exact identities,
all four normal/CPU launch/relaunch results and actual isolation/retirement controls
are in completion-evidence/final-matrix.json and selection-gate.md.
