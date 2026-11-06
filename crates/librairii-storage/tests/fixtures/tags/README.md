# Frozen Task 8 references

`manifest.json` records the exact sources, probes and outputs from frozen Go
`73a93303a9a44cf0a92eaf525331561ed8c04460`, executed with Go 1.26.5.
An independent repeated capture produced byte-identical outputs. Only synthetic
SQLite and explicit temporary paths are used; module downloads are disabled.
The admitted `preservation/` corpus is unchanged.

The probes are qualification tests, copied into `internal/tagging` in an isolated
`git archive` of that revision. Run `go test ./internal/tagging -run
'^TestTask8Frozen' -count=1 -v` with `GOPROXY=off`, `GOTOOLCHAIN=local`, and explicit
absolute `TASK8_NORMALIZATION`/`TASK8_VERTICAL` output paths. Do not overwrite these
references during routine checks. Neither probe changes original Go source.

`normalization.json` captures all 1,112,064 Unicode scalars through the actual
unexported Go tag-key normalizer, plus contextual and long-mark cases.
The digest frames each scalar as codepoint u32 LE, output-byte-length u32 LE,
valid byte, then output UTF-8 bytes. Rust compares the whole digest and explicit
cases. Frozen simple lowercase is deliberately distinct from search casefolding.
The Letter/Nd extraction recipe is `xtask/extract_tag_categories.py`, authenticated
against the original Unicode15 source in the existing source index; shared
lowercase/NFKD/mark/whitespace tables retain their admitted source and notices.

`vertical.json` records actual definition/value creation, mixed assignment
workspace, additive toggles, no-ops, invalid-target atomicity, stale deletion,
whole-Collection invalidation/revision, protected Age color seeding, independent
Broken batches and changed-only Undo. Rust compares every recorded field without
normalizing IDs, order, labels, flags or counts. The controlled SQL seeds run
through the actual unchanged 001–018 migrations.

The existing frozen Go tagging/library/metadata suites and both application
vertical tests also pass. Those results establish the reference behavior; they
are not Rust import, metadata transport, curation, exporter or GUI qualification.
