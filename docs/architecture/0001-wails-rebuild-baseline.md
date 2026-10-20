# Wails rebuild baseline and removal boundary

Status: accepted

Baseline commit: `9ed05ebb0672b9002e837312424c3d2bb12e0e86`

## Product and UI source of truth

Product behavior is documented in the [user guide](../user-guide.md) and
verified by application and release acceptance tests.
The revision notes below record implementation history. The current Collection,
Shelf, Broken-flag, and export policies are defined in
[Collections and curated Shelves](0004-collections-and-shelves.md).

The UI baseline is tracked in the repository:

- reference: `testdata/ui-prototypes/05-archive-shelves.html`
- SHA-256: `1b6bb40b2ca411726d90e942d4ea0b3620a5a0ec77b0976093a20be0a625f75a`
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
At widths of 850px or less, a Library & filters disclosure opens the existing
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

## Dedicated broken-story flag revision (2026-10-19, historical policies)

Migration 015 moves the retired built-in Broken assignments into
`stories.is_broken`, removes its definition and protection triggers, and records
the retired definition ID separately for legacy decoding. No tag key is reserved
for flag behavior. Startup seeds derived facets independently. Manual flags
survive metadata projection, import rediscovery, and archive verification.

Dedicated atomic set/restore operations accept explicit story states. Undo
checks target availability and expected current flags before committing the
whole restore. UI controls are separate from selection and tag editing, with
independent details state when a story disappears from the current result.

Collection URL version 2 and shelf payload version 4 separate root broken
visibility (hide/include/only) from explicit per-node flag membership. Older
payloads translate retired tag IDs before live reference validation; AND/OR
composition and even contradictory empty results are retained. Only old versions
interpret the compatibility identity, so a new tag can safely reuse its ID.
At that stage, current-query/shelf exports followed visibility and explicit
selections still allowed flagged stories. These policies were superseded by
the Collections and curated Shelves revision below.

## Collections and curated Shelves revision (2026-10-19)

Migrations 016–018 distinguish dynamic saved Collections from approved,
UUID-based Shelf membership. Collection sources produce review proposals;
membership changes require explicit approval. Missing stories retain their
Shelf identity and reconnect when their UUID returns.

Broken stories always remain visible and counted. The panel/details switch
and bulk actions update the independent flag with guarded Undo; story cards
show warnings. Every export skips Broken and unavailable members with a reason,
revalidates prepared eligibility, and checks queued stories before copying.
Supported legacy queries lose built-in Broken restrictions recursively.

Saved query version 5 and URL version 3 describe the current codecs. The
canonical snapshot includes separate Collections/Shelves navigation and no
Broken visibility selector. Documentation screenshots are captured from the
current frontend with [the fixture capture script](../assets/README.md).
