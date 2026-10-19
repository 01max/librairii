# Collections and curated Shelves verification

Change: `introduce-collections-and-curated-shelves` (44 implementation subtasks).
The OpenSpec artifacts and completion tracker live in the separate nested
`openspec` repository. The application contract is also tracked in
[0004-collections-and-shelves.md](../architecture/0004-collections-and-shelves.md).
Each subtask has one application commit, identified by its task number.

## Storage and upgrade

Migrations 016–018 rename saved queries to Collections, add UUID-based curated
membership/decisions, and extend immutable export records. Upgrade tests cover
versions 14 and 15, fresh initialization, historical source IDs, restart,
constraints, and automatic backup restoration on migration failure. Historical
`shelf`/`shelves` export identities retain their dynamic Collection meaning;
curated exports use separate discriminators.

Supported legacy saved-query version 1–4 and URL version 1–2 payloads normalize
into saved version 5 / URL version 3. Built-in Broken restrictions disappear
recursively, preserving ordinary tags and other predicate structure, including
empty OR alternatives. Unknown/corrupt definitions remain repairable. Existing
query memberships may deliberately broaden when retired flag restrictions vanish.

The user guide documents complete-library backups and backup-based downgrade
recovery. Successful upgrades have no reverse migration for curation decisions.

## Application and backend scenarios

`TestMigratedCollectionThroughShelfReviewRestartAndExport` starts with a real
version-14 SQLite library and managed ZIPs. It verifies flag migration and metadata
refresh, explicit seeds from a migrated Collection, protected manual/source
overlap, imported and tag-driven suggestions, exclusion reversal, approved
additions, removal/exclusion, unavailable placeholders, UUID reimport under a new
numeric ID, restart, and partial export with Broken/unavailable reasons.

Service suites cover deduplicated OR sources, permanent exclusions, deferral,
protection, invalid and failing sources, last-source detachment, atomic
multi-Shelf Collection deletion, rollback, concurrent apply, and stale snapshots.
Read previews never persist decisions. Tokens expire after 15 minutes and the
cache is bounded to 64 tickets. Flag-only changes do not invalidate curation.

Export suites exercise all scope kinds, unions, refined Shelf results, selected
unavailable UUIDs, all-ineligible scopes, prepared-state changes, per-queued-item
eligibility, cancellation, checksum preservation, and non-overwrite behavior.

## Browser and frontend checks

The scoped canonical snapshot and SHA-256 contract were refreshed together.
The browser compares the app and static reference at 1181, 1180, 850, and 560 px.
The original rail, story renderer, detail panel, and independent scrolling remain
in place. Collections/Shelves navigation and removal of the Broken selector are
part of the approved snapshot change.

Behavioral tests cover typed scope history and ID collisions, explicit creation,
manual additions, deferred review defaults, stale refresh, protection/exclusion
reversal, distinct export scopes, flag Undo, and retained cards/selection/focus.
English/French copy and French keyboard cancellation are covered.

The accessibility browser gate now exercises curated membership, a 300-candidate
review (25 mounted rows), source detachment, exclusions, and creation confirmation
at desktop and compact widths. It checks WCAG AA with axe, horizontal overflow,
keyboard cancellation, and focus return. Warning sets and Collection deletion
Shelves are also paginated. See the [performance baseline](../performance/large-library.md).

## Final release gate

Run from the application root:

```sh
make check
go test -race ./internal/...
openspec validate introduce-collections-and-curated-shelves --strict
```

`make check` includes frontend production build, Go formatting/vet/tests, TypeScript,
ESLint, frontend behavioral tests, canonical prototype contract, responsive and
pixel comparisons, accessibility, and the existing browser performance gate.
Bindings are generated with the pinned Wails CLI (`wails generate module`) and
normalized to file mode 0644. Automated browser checks use local preview servers.
Platform installer/device acceptance is separate from this feature's automated gate.

Final local evidence (2026-10-19, macOS arm64, Go 1.26.5):

- `make check` passed, including all 186 frontend tests, four zero-mismatch
  canonical comparisons, responsive layouts, and zero axe violations at both
  desktop and compact widths across the new curation dialogs.
- `go test -race ./internal/...` passed. An initial run alongside the normal Go
  suite hit the existing 750 ms browsing deadline; isolation and the full rerun
  passed without weakening that test or changing its deadline.
- Strict OpenSpec validation passed. Regenerating pinned Wails bindings produced
  no drift after file modes were normalized.
- Browser performance remained within budget (381.8 ms expansion p95, 3.8 ms
  input-delay p95 in the final gate). The earlier recorded baseline is retained
  as an observation, not a machine-independent latency requirement.
- Linux packaged installer evidence runs in CI; it was not executed on this
  macOS host. No device transfer acceptance is claimed.
