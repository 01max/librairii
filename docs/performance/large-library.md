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

## Interaction budgets

The measurement command exits unsuccessfully if any recorded p95 misses its
interaction budget:

| Scenario | p95 budget |
| --- | ---: |
| Collection query | 100 ms |
| Literal substring search | 120 ms |
| Combined filters | 150 ms |
| Six saved-shelf counts | 250 ms |
| Deep pagination | 120 ms |
| 24 artwork loads | 50 ms |

These budgets cover the synchronous Go/SQLite and local-file work beneath one
UI interaction. They retain substantial headroom over the reference machine
without pretending that wall-clock results are comparable across arbitrary
hosts.

## Query-plan and rendering decisions

- Compatibility filters use the covering
  `idx_story_archives_validation_story` index added by migration 013.
- Language, boolean, and choice predicates use their existing covering
  metadata and assignment indexes. Tests fail if those plans drift.
- Literal substring semantics intentionally retain an indexed normalized-name
  scan. SQLite's B-tree indexes cannot answer arbitrary infix matches, and the
  measured p95 remains well inside budget without changing search behavior.
- Shelf counts use a count-only library path, avoiding page-row and metadata
  hydration. This reduced the six-shelf p95 from 66.093 ms to 21.480 ms.
- “View all” fetches 100-story batches and publishes each batch between
  animation frames so React can paint and accept input during expansion.
- Collection artwork remains native-lazy and now requests asynchronous image
  decoding; only the selected drawer artwork is eager.
