# Frozen Task 9 reference

The manifest identifies exact source/probe/output bytes from frozen Go
`73a93303a9a44cf0a92eaf525331561ed8c04460` with Go 1.26.5. Two actual runs
produced byte-identical output. The original sources and `preservation/` corpus
are unchanged. Synthetic databases are created through unchanged migrations.

Copy `reference_probe.go.in` to `internal/collections/task9_reference_test.go` in
an isolated `git archive` of that revision. With `GOPROXY=off`,
`GOTOOLCHAIN=local` and an explicit absolute `TASK9_CAPTURE` output path, run
`go test ./internal/collections -run '^TestTask9FrozenReference$' -count=1`.
Routine tests read these captures; they never regenerate them.

`reference.json` contains exact public Collection fields for creation,
duplication, lazy inspection, renaming, normalized-name collision, replacement,
ordering, migrated ID 77 and unsupported ID 88. No IDs, revisions, query bytes,
positions or validity fields are removed/normalized in comparison. Collection
DTOs have no timestamp fields. Tests separately preserve raw invalid UTF-8 bytes.

The name digest covers every one of the 1,112,064 Unicode scalars through the
actual unexported Go normalizer. Each frame is codepoint u32 LE, display-byte
length u32 LE, normalized-byte length u32 LE, valid u8, then display and normalized
UTF-8 bytes. Invalid cases contribute empty strings. Contextual/control/length
cases are also captured. Rust compares all fields and the full digest.

The unchanged Go Collection suite and application Collection vertical slice pass.
These establish reference behavior, without claiming Rust import transport,
metadata transport, reviewed Shelf detachment, export execution or GUI readiness.
