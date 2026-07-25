# Wails rebuild baseline and removal boundary

Status: accepted for `rebuild-local-story-library-wails`

Baseline commit: `9ed05ebb0672b9002e837312424c3d2bb12e0e86`

## Product and UI source of truth

Functional scope comes only from:

- `openspec/changes/build-local-story-library/`
- `openspec/changes/rebuild-local-story-library-wails/`

The only UI source of truth is:

- `openspec/ui-prototypes/05-archive-shelves.html`
- SHA-256: `19119b85ed820e1893020347ad5015bbed173ef8c8e6e1164405d83f1b5f00f9`
- the `.app` subtree is normative
- the `.back` gallery link is excluded from the product

`prototype_contract_test.go` verifies the checksum and both subtree markers on
every Go test run. The frontend `?fixture=parity` bridge is deterministic and
records every sample shelf, cover title, count, selection, fact, tag, and
archive detail from that exact file.

The Rails screens and Tauri shell are implementation history, not product or
visual references.

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
