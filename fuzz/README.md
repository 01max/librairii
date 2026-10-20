# Isolated boundary fuzzing

Exact nightly2026-10-19 and cargo-fuzz0.13.2 are separate from normal stable builds.
`cargo xtask fuzz` runs the real identity boundary with retained synthetic seeds,
max_len128 and max_total_time30. No profiles, library data or endpoints are used.
Generated corpus additions and crash artifacts live under target/quality/fuzz;
retain minimized failures under fuzz/corpus/identity with a regression assertion.

Archive/JSON/query targets must call actual bounded product decoders after those
sections exist. They remain unimplemented; fuzzing a generic third-party parser
would not establish Librairii validation. This target is no archive or query pass.
No Miri/sanitizer claim is made for SQLite, GUI or native FFI.
