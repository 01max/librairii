# Large-library performance fixture

`go run ./cmd/library-performance -stories 5000 -samples 20` builds a fresh
temporary Librairii data root and records representative collection timings.
The generator creates deterministic synthetic titles, descriptions, UUIDs,
metadata, tag assignments, saved shelves, archive records, and an original
set of 24 deterministic 320×400 procedural PNG covers. It does not copy
copyrighted story or catalog content and it does not create usable
story-archive payloads.

The scenarios cover the first collection page, literal substring search,
combined name/language/compatibility/boolean/choice filters, all saved-shelf
counts, deep offset pagination, and loading the 24 distinct embedded covers
returned on the first page. Artwork timing exercises the application asset
handler, reads each distinct local file, and fully decodes every PNG at its
representative dimensions. Each metric is warmed once before the recorded
samples.

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
| 24 distinct asset-handler loads and PNG decodes | 50 ms |

These budgets cover the synchronous Go/SQLite and local-file work beneath one
UI interaction. They retain substantial headroom over the reference machine
without pretending that wall-clock results are comparable across arbitrary
hosts.

## Browser interaction budget

`npm --prefix frontend run test:performance` builds a production bundle with a
compile-time-only 1,000-story browser fixture, serves it from a temporary
directory, and drives the installed Chrome or Chromium through pinned
Playwright. The normal production build excludes the fixture. Set
`LIBRAIRII_CHROME_PATH` when the browser is not installed at a standard
platform path.

Five acceptance samples expand all 1,000 stories while real pointer input is
sent to the application. Expansion completion and pointer dispatch are
product-coupled acceptance gates. Animation-frame and timer scheduling remain
reported diagnostics because shared CI hosts can pause the entire browser
independently of application work:

| Browser scenario | Classification | p95 budget or threshold |
| --- | --- | ---: |
| Complete 1,000-story expansion | Acceptance gate | 3,000 ms |
| Pointer input delay | Acceptance gate | 50 ms |
| Animation frame gap | Scheduler diagnostic | 50 ms |
| Timer delay | Scheduler diagnostic | 50 ms |

The checked-in
[`frontend-large-library-baseline.json`](frontend-large-library-baseline.json)
records the current-host observation. The release gate compares each run to
the explicit acceptance budgets, not to another machine's wall-clock baseline.
Scheduler diagnostics retain explicit thresholds so regressions remain visible
without making shared-host pauses indistinguishable from application failures.

The browser scenario is intentionally separate from the 5,000-story
Go/SQLite fixture above. Five thousand records remain the storage, query,
filter, shelf, pagination, and artwork stress case; the UI gate models a
generous interactive library without requiring every synthetic backend row to
be mounted in the DOM simultaneously.

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
- “View all” fetches 50-story batches and publishes each batch between
  animation frames so React can paint and accept input during expansion.
- Collection artwork remains native-lazy and now requests asynchronous image
  decoding; only the selected drawer artwork is eager.
