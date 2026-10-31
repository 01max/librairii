# Documentation screenshots

## Native application

The images in `native/` are unmodified PNGs from actual packaged macOS ARM
windows in [native package CI run 41185064509](https://github.com/01max/librairii/actions/runs/41185064509).
They use disposable library/profile roots, synthetic archives and an isolated
loopback metadata fixture. No personal library or live Lunii catalog was used.
Captures include temporary CI paths and fixture timestamps; those are examples,
not locations to reuse. Linux shares the application controls but uses its own
system dialogs and file manager.

[`native/manifest.json`](native/manifest.json) binds every image to its archive
member, byte count and SHA-256, the provider artifact, inner package SHA-256,
target, PR head and actual build source/tree. These captures belong to PR69's
final head `21f32ed`, built as `bd7327b`; they are not evidence that a later
documentation head or public signed release was installed. The package is an
ad-hoc internal candidate; public signing/notarization/stapling remains gated.

| Native image | Captured state |
| --- | --- |
| `native/library.png` | List view after explicit fixture metadata sync, one selected story |
| `native/story-details.png` | Scrolled Details with provenance and technical disclosure |
| `native/tags-search.png` | Assigned user tag and read-only age values |
| `native/collections.png` | Saved Collection and union controls |
| `native/shelf-review.png` | Approved Shelf after explicit review, including acceptance attribution |
| `native/export.png` | Completed passthrough export report and Reveal destination |
| `native/settings.png` | Interface language, library folder, diagnostics and preference import |
| `native/history.png` | Persistent import, metadata and export history |

To refresh, use the existing disposable packaged-GUI CI workflow. Wait for the
complete run, retain both successes and failures, verify provider ZIP and inner
package checksums, and independently audit installed source/target/package
bindings plus the raw fresh/upgraded report seals. Copy selected PNG members
byte for byte, update the manifest and inspect every image/caption. Do not
replace a failed run with a success label or treat a screenshot as signature,
accessibility or full release qualification. No local native app, input driver,
observer or profiling is needed to extract/view retained PNG files.

## Historical Go/Wails interface

The top-level images below show the retained React application with synthetic frontend bridge
fixtures, generated placeholder covers, and no personal library data. They are
browser captures of the application, not native window or device screenshots.
They remain historical references; the Rust README/user guide uses `native/`.

From the repository root, after `make setup`, run:

```sh
npm --prefix frontend run docs:screenshots
```

Chrome or Chromium must be installed. Set `LIBRAIRII_CHROME_PATH` to its
executable if it is not in a standard platform location. The script builds into
a temporary directory with fixtures enabled, serves on a temporary loopback
port, and removes the build when finished. Normal production builds exclude
fixtures. It does not open the desktop app or read its SQLite library.

| Image | Fixture and state |
| --- | --- |
| `library.png` | `?fixture=parity`, Collections, Shelves, filters, selected story |
| `story-details.png` | `?fixture=parity`, Open details dialog |
| `tag-manager.png` | `?fixture=parity`, Manage tags dialog |
| `curated-shelf.png` | `?fixture=curated`, Weekend Shelf in List view with Broken and unavailable members |
| `shelf-review.png` | `?fixture=curated&review=removals`, automatic source-mismatch review with its decision deferred |
| `settings.png` | `?fixture=settings`, language and library location |

Capture settings: 1440 × 1000 viewport (1181 × 800 for the Shelf and review),
scale 1, English (`en-GB`), UTC,
reduced motion, and the frozen fixture clock (`2026-07-25T09:00:00Z`). Dialog
images are cropped to the dialog. Text and timestamps belong to fixtures, not a
live catalog sync. Fonts and browser versions can cause small platform differences.

Last refreshed: 2026-10-20. After regenerating, visually inspect every image and
check the README and user guide captions against the captured controls.
