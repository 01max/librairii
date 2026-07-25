# Large-library performance fixture

`go run ./cmd/library-performance -stories 5000 -samples 20` builds a fresh
temporary Librairii data root and records representative collection timings.
The generator creates deterministic synthetic titles, descriptions, UUIDs,
metadata, tag assignments, saved shelves, archive records, and an original
8×8 PNG. It does not copy copyrighted story or catalog content and it does not
create usable story-archive payloads.

The scenarios cover the first collection page, literal substring search,
combined name/language/compatibility/boolean/choice filters, all saved-shelf
counts, deep offset pagination, and loading the 24 embedded covers returned on
the first page. Each metric is warmed once before the recorded samples.

The checked-in baseline in
[`large-library-baseline.json`](large-library-baseline.json) was recorded on
2026-07-25 with Go 1.26.5 on macOS arm64. It is an observation for query-plan
and regression work, not a cross-machine benchmark comparison.
