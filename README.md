# Librairii

Librairii is a local-first desktop library for inspecting, organizing,
searching, shelving, and exporting Lunii story archives. It runs as a
single-process Wails, Go, and SQLite application: there is no sidecar service,
loopback web server, or external database.

The functional contract lives in the OpenSpec change
`rebuild-local-story-library-wails`. The exact visual contract is
`openspec/ui-prototypes/05-archive-shelves.html`. Clean checkouts verify
the checksum-locked mirror at
`testdata/ui-prototypes/05-archive-shelves.html`.

Start with the [user and recovery guide](docs/user-guide.md) for supported
formats, search and shelf behavior, export limitations, application data,
privacy, and Lunii.QT interoperability. See
[the rebuild baseline](docs/architecture/0001-wails-rebuild-baseline.md) for
the architecture boundary.

## Development

Install Go 1.25 or newer, Node 24, and the Wails CLI version recorded in
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

The first complete local story workflow has a synthetic, copyright-free
demonstration and automated smoke test:

```sh
make smoke-first-story
```

See [docs/demos/first-story-vertical-slice.md](docs/demos/first-story-vertical-slice.md)
for the packaged-app walkthrough.

## Release verification

On the current macOS host, build and independently verify the installer with:

```sh
make build-current-installer
make verify-current-installer
make smoke-release
make verify-packaged-acceptance
```

The packaged acceptance gate launches the installed application twice against
an isolated data root and covers the release UI, SQLite persistence, native
adapters, the complete local story workflow, and clean shutdown.

Windows and Linux priorities, artifacts, host requirements, and independent
verification commands are recorded in the
[release platform matrix](docs/release-platforms.md).
