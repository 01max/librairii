# Wails rebuild baseline and removal boundary

Status: accepted for `rebuild-local-story-library-wails`

Baseline commit: `9ed05ebb0672b9002e837312424c3d2bb12e0e86`

## Product and UI source of truth

Functional scope comes only from:

- `openspec/changes/build-local-story-library/`
- `openspec/changes/rebuild-local-story-library-wails/`

The only UI source of truth is:

- `openspec/ui-prototypes/05-archive-shelves.html`
- tracked CI mirror: `testdata/ui-prototypes/05-archive-shelves.html`
- SHA-256: `f39a5c74b6a10eb403c742b40306e1cf8d0cc38f73ac93d81e8272e23f6e42b4`
- the `.app` subtree is normative
- the `.back` gallery link is excluded from the product

`prototype_contract_test.go` verifies the tracked mirror's checksum and both
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
- all OpenSpec changes and prototypes
- any untracked application-data directory outside this repository

No removal command may target the repository root, the user home directory, or
an application-data directory. Legacy external data is never interpreted,
overwritten, or deleted by the rebuild.
