# Wails rebuild baseline and removal boundary

Status: accepted

Baseline commit: `9ed05ebb0672b9002e837312424c3d2bb12e0e86`

## Product and UI source of truth

Product behavior is documented in the [user guide](../user-guide.md) and
verified by application and release acceptance tests.

The UI baseline is tracked in the repository:

- reference: `testdata/ui-prototypes/05-archive-shelves.html`
- SHA-256: `a3709c344b0c0dd0af362e0bdc7be3d8892560c2aec8175e3f3e48b87832bd16`
- the `.app` subtree is normative
- the `.back` gallery link is excluded from the product

`prototype_contract_test.go` verifies the reference's checksum and both
subtree markers on every Go test run. The frontend `?fixture=parity` bridge is
deterministic and records every sample shelf, cover title, count, selection,
fact, tag, and archive detail from that exact file.

The Rails screens and Tauri shell are implementation history, not product or
visual references.

The 2026-10-14 user-requested browsing revision updates the visual contract for
labeled, working Library/Tags/Import/Export actions, the central official Lunii
sync prompt and last-successful-sync timestamp, and image-only story buttons
with external title captions. Image buttons follow intrinsic artwork ratios.
The original sidebar and drawer layout remain the baseline. Story details now
show artwork and readable descriptions, with technical data behind a toggle
and removal behind a separate confirmation.

The 2026-10-15 viewport revision bounds the application grid to the window
height and makes collection and filter panels scroll independently. The
nonfunctional profile badge is removed; the navigation remains visible while
either content panel scrolls.

The popup/tag revision treats broken-story inclusion as a query option applied
before pagination and counting, including shelf previews and current-view
exports. Summary records carry the broken state for accessible badges. Protected
tag identities remain fixed while colors are user-editable and preserved by
startup seeding and metadata projection. Migration 014 relaxes only color
updates and changes the previous default age color to green.

The 2026-10-19 browsing revision adds a persisted presentation preference for
text lists, compact cover grids, and the existing large cover grid. Presentation
does not enter saved queries or change selection, pagination, sorting, or exports.
At widths of 850px or less, a Shelves & filters disclosure opens the existing
sidebar as a scrollable modal panel with keyboard dismissal and focus return.
The previous breakpoint hid the sidebar without providing a reopening action.
Nested dialogs suspend the sidebar focus trap while retaining its original
disclosure as the focus return target until the sidebar closes.

The notice dismissal revision adds close controls to the Lunii sync panel and
finished-operation notices. Dismissed operation IDs and sync-panel visibility
are stored as local preferences and restored across app restarts; reconciliation
and new operations continue normally. A compact Show Lunii sync disclosure
restores the panel without triggering network work.
History reads the existing SQLite operation records in pages of 20 using a
creation-time/ID cursor, including reports whose notices were dismissed. Loading
history does not feed historical records into the collection notice stream.
The visual reference includes the view picker, History, and the narrow-window
disclosure.

## Tracked removal allowlist

The clean rebuild may remove these tracked legacy product paths:

- `.rspec`, `.rubocop.yml`, `.ruby-version`
- `Gemfile`, `Gemfile.lock`, `Rakefile`, `config.ru`
- `app/`
- `bin/`
- `config/`
- `db/`
- `lib/librairii/`
- `lib/tasks/`
- `log/`
- `package.json`, `package-lock.json`
- `packaging/`
- `public/`
- `script/`
- `spec/`
- `src-tauri/`
- `storage/`
- `vendor/`
- Rails/Tauri-only content in `.gitignore`, `.github/dependabot.yml`,
  `.github/workflows/ci.yml`, and `README.md`

## Preserved paths

The reset must preserve:

- `.git/` history and repository configuration
- `.github/` paths while replacing only legacy workflow content
- `.gitignore` while replacing legacy ignore rules
- `docs/`
- UI prototypes and their contract tests
- any untracked application-data directory outside this repository

No removal command may target the repository root, the user home directory, or
an application-data directory. Legacy external data is never interpreted,
overwritten, or deleted by the rebuild.
