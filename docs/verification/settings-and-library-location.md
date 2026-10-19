# Settings and library location verification

Change: `add-settings-and-library-location`.

Validation host: macOS on Apple silicon, 19 October 2026.

## Automated checks

- `make check`: frontend build, formatting, Go vet/tests, TypeScript, ESLint,
  frontend tests, responsive/visual/accessibility browser checks, and library
  performance checks.
- `go test -race . ./internal/preferences ./internal/platform ./internal/app`:
  application, profile, storage/lifecycle, and relocation tests.
- English/French catalogs have identical keys and interpolation parameters.
  Language switching preserves selection, navigation, user text, saved queries,
  and existing metadata locale policy. Unknown historical outcomes retain
  inspectable technical detail under a localized summary.
- French settings, setup, and relocation fixtures pass browser accessibility
  checks at desktop and narrow-window sizes.
- Restart tests cover pre-commit and post-commit relocation, corruption,
  cancellation, changed/unrecorded files, missing roots, uncertain preference
  publication, and retrying interrupted setup in its confirmed folder.
- Application integration covers import, relocation, current-root queries,
  stale export-preparation rejection, post-move export, embedded artwork,
  removal, logging, and diagnostics.

## Native and filesystem checks

`scripts/verify-settings-acceptance` passes against the packaged macOS app
bundle. It uses temporary HOME/XDG directories and temporary libraries, selects
folders through native panels, invokes the real application-menu Settings item
with its Command-comma accelerator, initializes a library, imports a fixture,
relocates it, and exports from the new root.

`TestCompleteLibraryMoveAcrossVolumes` also passes with
`LIBRAIRII_TEST_DESTINATION_ROOT` on a disposable mounted HFS+ disk image,
separate from the source filesystem. The image was detached and removed after
verification. Set that environment variable to a writable second local volume
to repeat the check; otherwise the test skips explicitly.

The Linux packaged verification script now runs the same isolated setup and
relocation scenario using GTK native dialogs and the Settings accelerator.
Linux native execution was not available on this macOS host and remains a
platform-specific verification item. Native acceptance must use the packaged
app; standalone macOS executable launches produced unreliable panel automation.

## Capability review

Strict OpenSpec validation passes for `add-settings-and-library-location`.
The implementation was checked against all four capability deltas:

| Capability | Verified behavior |
| --- | --- |
| Application settings | Native/in-app access, early-command acknowledgement, atomic independent preferences, first-run confirmation, legacy adoption, and isolated override launches |
| UI localization | English/French UI, immediate persisted switching, protected-label presentation, localized outcomes, and unchanged metadata/content policy |
| Library location | Native selection, destination validation, whole-library copy/verification, exclusive lifecycle access, cancellation, restart recovery, and identity-checked missing-library recovery |
| Local story catalog | User-confirmed new-library initialization and preservation of configured or existing libraries without silent empty replacement |

All 41 implementation subtasks are complete. Linux native execution remains the
host-specific limitation recorded above; its acceptance scenario is wired into
the Linux verification script.
