# Dedicated broken-story flag verification

Change: `promote-broken-to-story-flag` (27 implementation subtasks).

The product now stores Broken on the story and offers direct list/cover flag
actions, separate panel/details warnings, bulk setting/clearing, and guarded
Undo. Hidden-by-default visibility, Include broken stories, and Broken only
apply before counts and pagination. Export scope behavior is preserved.

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

Final command results and the migrated-library walkthrough are recorded below
as their implementation subtasks complete.
