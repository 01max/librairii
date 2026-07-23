# Librairii

A local story-library companion for Lunii.QT, built with Ruby on Rails, SQLite,
Hotwire, and Tauri.

## Why this stack

A Rails server packaged inside a Tauri application is not the smallest or most
performance-efficient desktop architecture. It adds startup time, memory use,
and package size compared with a native Rust backend. That trade-off is
intentional: Librairii is a small weekend project, and Ruby/Rails is the stack I
am most comfortable and efficient with. Fast iteration and an enjoyable,
maintainable codebase matter more here than theoretical runtime efficiency; any
real performance problem should be measured before it justifies a rewrite.

## Development

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
bin/build
npm run package:smoke
```
