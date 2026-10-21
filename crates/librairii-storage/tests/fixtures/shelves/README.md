# Frozen Task 10 Shelf reference

The probe runs against the actual unchanged Shelf and Collection services at
`73a93303a9a44cf0a92eaf525331561ed8c04460` using Go 1.26.5. Two independent
runs produced byte-identical output. Source, probe and output hashes are in
`manifest.json`; the unchanged Go Shelf and Collection suites also pass.

In an isolated `git archive` of that revision, copy `reference_probe.go.in` to
`internal/shelves/task10_reference_test.go`. Set `GOPROXY=off`,
`GOTOOLCHAIN=local`, and `TASK10_CAPTURE` to an absolute output path, then run
`go test ./internal/shelves -run '^TestTask10FrozenReference$' -count=1`.
Routine Rust verification only reads the capture.

Every Shelf summary field is compared: ID, name, position, revision, approved,
Broken, unavailable and active-source counts. Member snapshots compare UUID,
raw stored position, protection, cached title and every historical origin field.
Source snapshots compare every source ID, Collection ID (zero when absent), name
and active flag. Exclusions compare ordered UUIDs. Preview comparisons include
UUID, title, manual seed flag, current sources and historical origins. Token
nonces, timestamp-bearing story preview DTOs and explanatory prose are outside
this capture; separate Rust tests exercise snapshot/ticket and display behavior.

The flow covers mixed seeding, overlap, Not now, exclusion, ordinary removal,
source readdition, Keep, unavailable members, Broken, independent duplication
and last-source detachment. It does not regenerate preservation fixtures.
