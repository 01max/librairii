# ADR 0001: Package an embedded Ruby runtime behind one sidecar launcher

## Status

Accepted for the macOS foundation spike.

## Context

The desktop application must start Rails without asking the user to install
Ruby, Bundler, Rails, SQLite, or a shell environment. Ruby 4.0.6 must remain the
same in development and in the packaged application. Single-file Ruby packers
either lag current Ruby releases or obscure the native-library boundary that
this early spike is intended to prove.

## Decision

Build Ruby 4.0.6 with `--enable-load-relative`, install production gems into a
staged Rails tree, and bundle both trees as Tauri resources. The build vendors
non-system macOS dynamic libraries and rewrites their load commands to an
`@rpath` rooted beside the embedded Ruby executable.

Tauri launches one executable resource, `librairii-backend`. That launcher sets
the isolated Bundler, Bootsnap, application-data, port, and launch-secret
environment and then uses `exec` to replace itself with the embedded Ruby/Rails
process. Tauri therefore supervises one stable child PID and does not depend on
the user's `PATH` or installed Rubies.

The current spike packages only the current `aarch64-apple-darwin` host. Each
future target must build its own Ruby and native gems and pass the same smoke
test; these artifacts are never shared across operating systems or CPU targets.
Tauri applies an ad-hoc signature to current-host development bundles after all
resources are embedded. A distribution identity and notarization remain release
work, but the generated `.app` passes strict deep signature verification.

## Build and verification

```sh
script/build_packaged_runtime
npm run tauri:build
script/smoke_packaged_sidecar
```

The smoke test launches the sidecar from the generated `.app` with a sanitized
environment, authenticates the loopback health endpoint, verifies a persistent
SQLite database was created below a temporary application-data root and passes
an integrity check, terminates the child, and confirms the listener is gone.
Set `LIBRAIRII_REBUILD_RUBY=1` for a clean embedded-Ruby and gem rebuild, or
`LIBRAIRII_REBUILD_GEMS=1` to rebuild only the staged production bundle.

## Consequences

- End users need no Ruby or Rails setup.
- The `.app` is larger because it contains Ruby, Rails, native gems, and their
  non-system libraries.
- Builds remain inspectable and incremental: the runtime and staged Rails tree
  are ignored build artifacts produced by a committed script.
- macOS signing and every additional target must include and verify all runtime
  resources independently.
