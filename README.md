# Librairii

A local story-library companion for Lunii.QT, built with Ruby on Rails, SQLite,
Hotwire, and Tauri.

## Development

The project uses Ruby 4.0.6, Rails 8.1.3, Tauri 2.11.5, and Tauri CLI 2.11.4.

```sh
bin/setup
bin/dev
```

Run the RSpec suite with:

```sh
bundle exec rspec
```

Rails keeps development data below `tmp/librairii/development` and test data
below `tmp/librairii/test`. Set `LIBRAIRII_DATA_ROOT` to use an explicit root;
the desktop shell sets it to the operating system's application-data directory.

Plain Rails development (`bundle exec rails s`) accepts unauthenticated loopback
requests. When `LIBRAIRII_LAUNCH_SECRET` is set, the server requires it;
`bin/dev` prints a one-time authenticated browser URL and the desktop shell
supplies its own secret.

Launch the Tauri development shell with:

```sh
npm run tauri:dev
```

The npm Tauri commands locate Cargo through rustup when its proxy directory is
not already present in the shell `PATH`.

## Current-host package smoke

Build the embedded Ruby/Rails runtime, create the signed macOS application, and
exercise the packaged sidecar without a user-installed Ruby:

```sh
npm run package:runtime
npm run tauri:build
npm run package:smoke
```

The packaging mechanism and its current-platform limits are recorded in
`docs/adr/0001-embedded-ruby-sidecar.md`.
