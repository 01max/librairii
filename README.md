# Librairii

Librairii is being rebuilt as a single-process Wails, Go, and SQLite desktop
application.

The functional contract lives in the OpenSpec change
`rebuild-local-story-library-wails`. The exact visual contract is
`openspec/ui-prototypes/05-archive-shelves.html`.

See `docs/architecture/0001-wails-rebuild-baseline.md` for the clean-rebuild
boundary.

## Development

Install Go 1.26, Node 24, and the Wails CLI version recorded in
`.wails-version`, then run:

```sh
make setup
make check
make build
```

`make check` formats and vets Go, runs Go and frontend tests, type-checks and
lints TypeScript, and builds the production frontend.

Run the current-macOS clean-environment launch, SQLite reopen, and shutdown
smoke with:

```sh
make smoke-foundation
```
