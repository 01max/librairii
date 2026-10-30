# Moving from Go Librairii to the native application

These instructions apply to the Rust rewrite branch. Final signed packages and
installed-platform qualification are handled separately; see
[the rewrite checklist](rewrite/section-17.md).

1. Finish or cancel active imports, exports and metadata sync in Go. In Settings,
   select **Export preferences…** (**Exporter les préférences…**) and choose a
   folder without an existing transfer. Keep `librairii-preferences.json`.
2. Quit Go completely. Copy the **entire library folder** and its separate Go
   settings folder to a backup location. Include the database, archives, artwork,
   catalog, history, staging, trash, logs, additional root files, settings and
   any recovery journal. Keep the original folder paths with this backup.
   A database file copied while an application is running is insufficient.
3. If a move or recovery is pending, reconnect the expected drive and finish
   recovery in the owning application first. Keep unfinished copies and journals.
   Do not delete SQLite sidecars, lock files or recovery records to force opening.
4. Start native Librairii and confirm a **different empty folder** for its verified
   independent copy. Review the exact folder. The existing Go settings and
   default location continue to select the original Go source. Native profiles,
   recovery records and copied files belong to the separate candidate.
5. In native Settings, select **Import preferences…** (**Importer les préférences…**)
   and choose the exported JSON file. It must belong to this library. Import
   once, before customizing native preferences, to transfer all available choices.
   Already saved native choices take precedence. Repeating an import preserves
   current choices; another export cannot reset them.
6. Check your stories, Collections, Shelves, tags, Broken flags, History and
   presentation after closing and reopening native Librairii. Keep the source
   and backup until you have checked the result.

The transfer contains view mode, dismissed notice identifiers, sync-panel
visibility and bounded semantic navigation. It excludes browser storage, tokens,
private paths, archive data and the database. Navigation includes only browser
history known to that Go session; earlier history cannot be enumerated after a
reload. Re-export in a new folder if you want a fresh transfer. Copying the JSON
alone does not back up a library.

Without a transfer, library data still moves through the verified copy. Native
presentation starts in List view at Library, with default query settings, no
imported dismissed notices and the sync panel visible when applicable. Language
comes from the independent saved profile. An invalid or future transfer remains
untouched and leaves existing choices intact.

## Returning to Go

Use the original Go source or a **separate restored pre-native backup**. Never
restore an older database over the newer native candidate. The bridge requires
this separate rollback route and refuses a native-owned library. The disposable
reopening rehearsal preserves the tested Collections, Shelves and native edits,
but Go startup refreshes derived search names and may add derived Age tags.
That bounded experiment does not qualify arbitrary native changes for direct
rollback. Keep the native recovery marker intact. Older Go executables can ignore the shared lock;
keep their settings pointed at the original source.

Changes made after the backup remain in the native candidate. Before switching,
keep a full closed copy of that candidate and its native settings. Export newly
imported archives to a separate destination and re-import them into the restored
Go copy if needed. Review Collection criteria, Shelf order/membership, exclusions,
source decisions, tags, Broken flags and notice preferences explicitly. Those
organizational edits are not merged automatically by archive export. Record which
application contains each newer change before resuming work.

## If opening or recovery is refused

Close the other application and retry. Reconnect the expected drive or locate the
same library UUID through native recovery. A different library cannot replace it.
Keep saved settings, journals, source and unfinished destination together. Native
recovery follows its own verified record; an imported Go journal never authorizes
native cleanup. Changed or unrelated files are retained. You can change language
while storage is unavailable. Keep the retained paths shown by recovery; do not
manually remove control files or overwrite uncertain data.
