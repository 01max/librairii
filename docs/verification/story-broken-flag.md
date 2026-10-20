# Dedicated broken-story flag verification

> Historical verification record for the original Broken change. Its hiding and
> export policies were superseded by [Collections and curated Shelves](collections-and-shelves.md).
> The integration test was extended and renamed to
> `TestMigratedCollectionThroughShelfReviewRestartAndExport`; the current user guide
> describes always-visible Broken stories and universal export skips.


Change: `promote-broken-to-story-flag` (27 implementation subtasks).

The original implementation stored Broken on the story and offered direct
list/cover flag actions, separate panel/details warnings, bulk setting/clearing,
and guarded Undo. At that stage, hidden-by-default visibility, Include broken
stories, and Broken only applied before counts and pagination; export scope
behavior was preserved. These visibility and export policies are historical.

## Automated behavior coverage

- Database upgrade preserves old assignments, unrelated manual/derived tags,
  and restart state; injected migration failure restores the existing backup.
- Go/TypeScript codecs cover current visibility, explicit flag membership,
  supported legacy URL/shelf versions, nested alternatives, contradictory empty
  results, and retired ID reuse without confusing an ordinary broken-named tag.
- Atomic flag operations cover single/mixed batches, deduplication, no-ops,
  invalid/unavailable targets, write rollback, readiness, exact restoration, and
  stale/removed Undo targets.
- UI tests cover every index view, shelf-preview duplicates, keyboard access,
  per-card targeting during bulk selection, pending/error/no-op outcomes,
  disappearing results, final-page reconciliation, and retained details.
- Export tests use real SQLite and managed archive fixtures to check visibility,
  migrated composed shelves, union deduplication, explicitly flagged selection,
  preflight, and unchanged prepared snapshots.
- Metadata refresh preserves the independent manual flag.

## Browser verification

The responsive suite exercises direct keyboard flagging and Undo in list,
small-cover, and large-cover views; Broken only clearing; and touch flagging
without hover. It also checks short/narrow windows and French controls.

Canonical screenshot comparison passes at 1181, 1180, 850, and 560px widths.
The largest mismatch is 0.031%, within the existing 0.1% tolerance. Accessibility
checks report zero axe violations at 1181px and 560px, no positive tab indexes,
and no unlabeled tags or shelves. Reduced motion remains respected.

Canonical snapshot: `testdata/ui-prototypes/05-archive-shelves.html`.
The checksum contract and both parity fixtures identify the updated snapshot.

## Final checks

On 2026-10-19, `make check` passed: frontend build, Go formatting and vet,
all Go tests, TypeScript, ESLint, prototype checksum, all 165 frontend unit
tests, responsive browser checks, visual parity, accessibility, and performance.
The separately exercised `make` targets use those same browser scripts.

The 1,000-story browser performance run stayed within budget: expansion p95
388 ms (3,000 ms budget), input delay p95 4 ms (100 ms budget), and frame gap
p95 16.7 ms. The 5,000-story SQLite fixture retains indexed archive, language,
and tag-value lookups without adding a low-selectivity flag index.

## Migrated-library walkthrough

`TestMigratedBrokenLibraryThroughApplicationRestartRefreshAndExport` passes
against real SQLite and managed ZIP archives. It constructs a version-14
library with a customized Broken tag and a composed saved shelf, upgrades it,
and verifies the retained compatibility identity and equivalent shelf results.
It then marks an unflagged story, checks default disappearance, undoes it,
clears the migrated flag from Broken only, and undoes that change.

After closing and reopening the database and application runtime, it refreshes
metadata from a local catalog fixture, verifies that the story gains official
metadata while retaining its manual flag, and exports the explicitly selected
flagged story through preflight and the durable export operation. Exported
bytes match the original managed ZIP exactly. Neither restart nor refresh
recreates the retired tag.

Index interaction, disappearance/Undo, and the three view modes are exercised
through the real browser against the stateful development fixture; the above
application integration test verifies the same operations with migrated storage.
The complete Go suite passes again after adding this walkthrough test.
OpenSpec strict validation passes and all 27 subtasks are complete.
