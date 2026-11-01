# Isolated boundary fuzzing

Exact nightly2026-10-19 and cargo-fuzz0.13.2 are separate from normal stable builds.
`cargo xtask fuzz` runs both actual product boundaries for30seconds each with
seed1, a2second per-input timeout and512MiB RSS ceiling. Retained synthetic seeds
are copied to target/quality/fuzz/corpus/<target>; generated additions and crash
artifacts stay under target/quality/fuzz. Minimized failures must be retained with
a regression assertion. No profiles, library data or endpoints are used.

`identity` keeps max_len128. `import_boundaries` uses max_len16384 and dispatches
ZIP/7z headers, path validation, Studio JSON and optional metadata JSON through
the actual product decoders. Archive limits are reduced to64entries,128KiB
expansion,16KiB metadata/art and16KiB headers/8MiB decoder memory. The public
bounded constructor can only reduce production ceilings. This exercises actual
streaming decoders, not a replacement generic parser. Production ceilings and
larger hardening counterexamples also have native regression coverage.

No Miri/sanitizer claim is made for SQLite, GUI or native FFI.
