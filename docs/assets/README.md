# Documentation screenshots

These images show the current React application with synthetic frontend bridge
fixtures, generated placeholder covers, and no personal library data. They are
browser captures of the application, not native window or device screenshots.

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
